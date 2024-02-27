use std::{
    collections::BTreeMap,
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    ash_immediate::{AshImmediateCommandBufferHandler, ImmediateRenderingFrameTask},
    backend_shared::BackendShared,
    compiled_frame_graph::CompiledFrameGraph,
    persistent_frame_state::PersistentFrameState,
    presenter_shared::PresenterShared,
};

use jeriya_backend::{
    immediate::ImmediateRenderingFrame, instances::camera_instance::CameraInstance, resources::ResourceEvent, transactions::Transaction,
};
use jeriya_backend_ash_base::{fence::Fence, semaphore::Semaphore, surface::Surface, swapchain_vec::SwapchainVec};
use jeriya_content::{asset_importer::Asset, shader::ShaderAsset};
use jeriya_macros::profile;
use jeriya_shared::{
    debug_info,
    log::{info, trace},
    parking_lot::Mutex,
    spin_sleep_util,
    tracy_client::Client,
    winit::window::WindowId,
    EventQueue, FrameRate,
};

pub enum PresenterEvent {
    RenderImmediateCommandBuffer {
        immediate_command_buffer_handler: AshImmediateCommandBufferHandler,
        immediate_rendering_frame: ImmediateRenderingFrame,
    },
    ProcessTransaction(Transaction),
    ShaderImported(Asset<ShaderAsset>),
}

pub struct Presenter {
    _presenter_index: usize,
    _thread: JoinHandle<()>,
    event_queue: Arc<Mutex<EventQueue<PresenterEvent>>>,
    presenter_shared: Arc<Mutex<PresenterShared>>,
}

#[profile]
impl Presenter {
    /// Creates a new `Presenter` and spawns a thread for it.
    pub fn new(
        presenter_index: usize,
        window_id: WindowId,
        backend_shared: Arc<BackendShared>,
        frame_rate: FrameRate,
        surface: &Arc<Surface>,
    ) -> jeriya_backend::Result<Self> {
        let presenter_shared = Arc::new(Mutex::new(PresenterShared::new(&window_id, &backend_shared, surface)?));
        let presenter_shared2 = presenter_shared.clone();
        let event_queue = Arc::new(Mutex::new(EventQueue::new()));
        let event_queue2 = event_queue.clone();
        let thread = thread::Builder::new()
            .name(format!("presenter-thread-{presenter_index}"))
            .spawn(move || {
                if let Err(err) = run_presenter_thread(
                    presenter_index,
                    backend_shared,
                    presenter_shared2,
                    window_id,
                    frame_rate,
                    event_queue2,
                ) {
                    panic!("Error on PresenterThread {presenter_index} (Window: {window_id:?}): {err:?}");
                }
            })
            .expect("failed to spawn presenter thread");

        Ok(Self {
            _presenter_index: presenter_index,
            _thread: thread,
            event_queue,
            presenter_shared,
        })
    }

    /// Sends a [`PresenterEvent`] to the presenter thread.
    pub fn send(&self, event: PresenterEvent) {
        self.event_queue.lock().push(event);
    }

    /// Returns the index of the presenter
    pub fn presenter_index(&self) -> usize {
        self._presenter_index
    }

    /// Sets the active camera
    pub fn set_active_camera(&self, camera_instance: &CameraInstance) {
        self.presenter_shared.lock().active_camera_instance = Some(*camera_instance.gpu_index_allocation());
    }
}

fn run_presenter_thread(
    presenter_index: usize,
    backend_shared: Arc<BackendShared>,
    presenter_shared: Arc<Mutex<PresenterShared>>,
    window_id: WindowId,
    frame_rate: FrameRate,
    event_queue: Arc<Mutex<EventQueue<PresenterEvent>>>,
) -> jeriya_backend::Result<()> {
    // Setup Tracy profiling
    #[rustfmt::skip]
    const PRESENTER_NAMES: [&str; 8] = [
        "presenter_thread_0", "presenter_thread_1", "presenter_thread_2", "presenter_thread_3",
        "presenter_thread_4", "presenter_thread_5", "presenter_thread_6", "presenter_thread_unknown",
    ];
    let name = PRESENTER_NAMES[presenter_index.min(PRESENTER_NAMES.len() - 1)];
    let client = Client::start();
    client.set_thread_name(name);

    let mut persistent_frame_states = SwapchainVec::new(&presenter_shared.lock().swapchain, |_| {
        PersistentFrameState::new(presenter_index, &window_id, &backend_shared)
    })?;
    let mut compiled_frame_graphs: SwapchainVec<Option<CompiledFrameGraph>> =
        SwapchainVec::new(&presenter_shared.lock().swapchain, |_| Ok(None))?;

    // Immediate rendering frames
    //
    // The immediate rendering frames are stored per update loop name. When a newer frame is received, the
    // current one is replaced. When a command buffer in received that belongs to the current frame, it is
    // inserted into the `ImmediateRenderingFrameTask`.
    let mut immediate_rendering_frames = BTreeMap::<&'static str, ImmediateRenderingFrameTask>::new();

    let mut interval = match frame_rate {
        FrameRate::Limited(frame_rate) => Some(spin_sleep_util::interval(Duration::from_secs_f32(1.0 / frame_rate as f32))),
        FrameRate::Unlimited => None,
    };

    info!("Starting presenter loop with frame rate: {:?}", frame_rate);
    loop {
        let mut presenter_shared = presenter_shared.lock();

        // Set the swapchain index to None to indicate that the swapchain image is not yet determined
        presenter_shared.frame_index.set_swapchain_index(None);

        backend_shared
            .resource_event_sender
            .send(ResourceEvent::FrameStart)
            .expect("failed to send ResourceEvent::FrameStart");

        // Remove timed out immediate rendering frames.
        //
        // The immediate rendering frames are removed one frame after they timed out. This is to make sure that
        // no flickering accurs when the timeout is set exactly to the frame rate. The flickering seems to occur
        // due to inconsistencies in the frame rate of the update and render loops. It is not occuring when the
        // timeout is set to Timeout::Infinite. (When the command buffers are rendered until they are replaced.)
        immediate_rendering_frames.retain(|_, task| !task.is_timed_out);
        for task in immediate_rendering_frames.values_mut() {
            task.is_timed_out = task.immediate_rendering_frame.timeout().is_timed_out(&task.start_time);
        }

        // Handle the incoming events for the presenter
        let mut event_queue = event_queue.lock().take();
        while let Some(new_events) = event_queue.pop() {
            match new_events {
                PresenterEvent::RenderImmediateCommandBuffer {
                    immediate_command_buffer_handler,
                    immediate_rendering_frame,
                } => {
                    // Check if the newly received immediate rendering frame is newer than the one that is already rendering
                    let mut remove_frame = false;
                    if let Some(task) = immediate_rendering_frames.get_mut(&immediate_rendering_frame.update_loop_name()) {
                        if immediate_rendering_frame.index() > task.immediate_rendering_frame.index() {
                            remove_frame = true;
                        }
                    }
                    if remove_frame {
                        immediate_rendering_frames.remove(&immediate_rendering_frame.update_loop_name());
                    }

                    // Insert the newly received command buffer
                    if let Some(task) = immediate_rendering_frames.get_mut(&immediate_rendering_frame.update_loop_name()) {
                        task.immediate_command_buffer_handlers.push(immediate_command_buffer_handler);
                    } else {
                        let task = ImmediateRenderingFrameTask {
                            start_time: Instant::now(),
                            is_timed_out: false,
                            immediate_rendering_frame: immediate_rendering_frame.clone(),
                            immediate_command_buffer_handlers: vec![immediate_command_buffer_handler],
                        };
                        immediate_rendering_frames.insert(immediate_rendering_frame.update_loop_name(), task);
                    }
                }
                PresenterEvent::ProcessTransaction(transaction) => {
                    let len = persistent_frame_states.len();
                    for (index, frame) in persistent_frame_states.iter_mut().enumerate() {
                        if index == len - 1 {
                            frame.push_transaction(transaction);
                            break;
                        }
                        frame.push_transaction(transaction.clone());
                    }
                }
                PresenterEvent::ShaderImported(shader_asset) => presenter_shared.vulkan_resource_coordinator.update_shader(shader_asset)?,
            }
        }

        // Finish command buffer execution
        let mut queues = backend_shared.queue_scheduler.queues();
        queues.presentation_queue(window_id).poll_completed_fences()?;
        drop(queues);

        // Render the frame
        match CompiledFrameGraph::new(&mut presenter_shared) {
            Ok(mut compiled_frame_graph) => {
                // Setup synchronization primitives for the next frame
                let image_available_semaphore = Semaphore::new(&backend_shared.device, debug_info!("image-available-Semaphore"))?;
                let rendering_complete_semaphore = Semaphore::new(&backend_shared.device, debug_info!("rendering-complete-Semaphore"))?;
                let rendering_complete_fence = Fence::new(&backend_shared.device, debug_info!("rendering-complete-Fence"))?;

                // Acquire the next swapchain image
                let acquire_span = jeriya_shared::span!("acquire swapchain image");
                let swapchain_image_index = loop {
                    match presenter_shared.swapchain.acquire_next_image(&image_available_semaphore) {
                        Ok(index) => break index,
                        Err(_) => {
                            info!("Failed to acquire next swapchain image. Recreating swapchain.");
                            presenter_shared.recreate(&backend_shared)?;
                        }
                    }
                };
                presenter_shared.frame_index.set_swapchain_index(swapchain_image_index as usize);
                drop(acquire_span);

                let persistent_frame_state = persistent_frame_states.get_mut(&presenter_shared.frame_index);

                let wait_span = jeriya_shared::span!("wait for rendering complete");
                persistent_frame_state.rendering_complete_fence.wait()?;
                drop(wait_span);

                // Process Transactions which update the persistent frame state
                persistent_frame_state.process_transactions()?;

                // Reset CommandPool
                persistent_frame_state.command_pool.reset()?;

                // Free the frame graph of the frame that was previously rendered in this position
                let previous_frame_graph = compiled_frame_graphs.get_mut(&presenter_shared.frame_index).take();
                drop(previous_frame_graph);

                // Update the synchronization primitives for the next frame
                persistent_frame_state.image_available_semaphore = image_available_semaphore;
                persistent_frame_state.rendering_complete_semaphore = rendering_complete_semaphore;
                persistent_frame_state.rendering_complete_fence = rendering_complete_fence;

                compiled_frame_graph.execute(
                    persistent_frame_state,
                    &window_id,
                    &backend_shared,
                    &mut presenter_shared,
                    &immediate_rendering_frames,
                )?;
                // Set the compiled frame graph for the current frame
                assert!(compiled_frame_graphs.get(&presenter_shared.frame_index).is_none());
                compiled_frame_graphs
                    .get_mut(&presenter_shared.frame_index)
                    .replace(compiled_frame_graph);

                // Present
                let mut queues = backend_shared.queue_scheduler.queues();
                let result = presenter_shared.swapchain.present(
                    &presenter_shared.frame_index,
                    &persistent_frame_state.rendering_complete_semaphore,
                    queues.presentation_queue(window_id),
                );
                // The queues must be dropped before `recreate` is called to prevent a deadlock.
                drop(queues);
                match result {
                    Ok(is_suboptimal) => {
                        if is_suboptimal {
                            info!("Swapchain is suboptimal. Recreating swapchain.");
                            presenter_shared.recreate(&backend_shared)?;
                        }
                    }
                    Err(_err) => {
                        info!("Failed to present swapchain image. Recreating swapchain.");
                        presenter_shared.recreate(&backend_shared)?;
                    }
                }
            }
            Err(err) => {
                trace!("Failed to compile frame graph: {err:?}");
            }
        }

        presenter_shared.frame_index.increment();

        drop(presenter_shared);

        if let Some(interval) = &mut interval {
            interval.tick();
        }
    }
}
