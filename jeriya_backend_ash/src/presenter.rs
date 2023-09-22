use std::{
    collections::BTreeMap,
    sync::Arc,
    thread::{self, JoinHandle},
    time::Instant,
};

use crate::{
    ash_immediate::{AshImmediateCommandBufferHandler, ImmediateRenderingFrameTask},
    backend_shared::BackendShared,
    frame::Frame,
    presenter_shared::PresenterShared,
};

use jeriya_backend::{immediate::ImmediateRenderingFrame, ResourceEvent};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{semaphore::Semaphore, surface::Surface, swapchain_vec::SwapchainVec};
use jeriya_macros::profile;
use jeriya_shared::{
    debug_info,
    log::{info, trace},
    parking_lot::Mutex,
    spin_sleep,
    tracy_client::{span, Client},
    winit::window::WindowId,
    EventQueue, FrameRate, Handle,
};

#[derive(Debug)]
pub enum PresenterEvent {
    Recreate,
    RenderImmediateCommandBuffer {
        immediate_command_buffer_handler: AshImmediateCommandBufferHandler,
        immediate_rendering_frame: ImmediateRenderingFrame,
    },
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

    /// Recreates the [`PresenterShared`] in case of a swapchain resize
    pub fn recreate(&self) -> base::Result<()> {
        self.send(PresenterEvent::Recreate);
        Ok(())
    }

    /// Sets the active camera
    pub fn set_active_camera(&self, handle: Handle<jeriya_backend::Camera>) {
        self.presenter_shared.lock().active_camera = handle;
    }

    /// Returns the active camera
    pub fn active_camera(&self) -> Handle<jeriya_backend::Camera> {
        self.presenter_shared.lock().active_camera.clone()
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
    const PRESENTER_NAMES: [&'static str; 8] = [
        "presenter_thread_0", "presenter_thread_1", "presenter_thread_2", "presenter_thread_3",
        "presenter_thread_4", "presenter_thread_5", "presenter_thread_6", "presenter_thread_unknown",
    ];
    let name = PRESENTER_NAMES[presenter_index.min(PRESENTER_NAMES.len() - 1)];
    let client = Client::start();
    client.set_thread_name(name);

    let mut frames = SwapchainVec::new(presenter_shared.lock().swapchain(), |_| {
        Frame::new(presenter_index, &window_id, &backend_shared)
    })?;

    // Immediate rendering frames
    //
    // The immediate rendering frames are stored per update loop name. When a newer frame is received, the
    // current one is replaced. When a command buffer in received that belongs to the current frame, it is
    // inserted into the `ImmediateRenderingFrameTask`.
    let mut immediate_rendering_frames = BTreeMap::<&'static str, ImmediateRenderingFrameTask>::new();

    // Command Buffer that is checked to determine whether the rendering is complete
    let mut rendering_complete_command_buffer = SwapchainVec::new(&presenter_shared.lock().swapchain(), |_| Ok(None))?;

    let mut loop_helper = match frame_rate {
        FrameRate::Unlimited => spin_sleep::LoopHelper::builder().build_without_target_rate(),
        FrameRate::Limited(frame_rate) => spin_sleep::LoopHelper::builder().build_with_target_rate(frame_rate as f64),
    };

    info!("Starting presenter loop with frame rate: {:?}", frame_rate);
    loop {
        loop_helper.loop_start();

        let mut presenter_shared = presenter_shared.lock();

        backend_shared
            .resource_sender
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
                PresenterEvent::Recreate => {
                    presenter_shared.recreate(&window_id, &backend_shared)?;
                }
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
            }
        }

        // Finish command buffer execution
        let mut queues = backend_shared.queue_scheduler.queues();
        queues.presentation_queue(window_id).poll_completed_fences()?;
        drop(queues);

        // Acquire the next swapchain image
        let acquire_span = span!("acquire swapchain image");
        let image_available_semaphore = Arc::new(Semaphore::new(&backend_shared.device, debug_info!("image-available-Semaphore"))?);
        let frame_index = loop {
            match presenter_shared
                .swapchain()
                .acquire_next_image(&presenter_shared.frame_index, &image_available_semaphore)
            {
                Ok(index) => break index,
                Err(_) => {
                    info!("Failed to acquire next swapchain image. Recreating swapchain.");
                    presenter_shared.recreate(&window_id, &backend_shared)?;
                }
            }
        };
        presenter_shared.frame_index = frame_index.clone();
        frames
            .get_mut(&presenter_shared.frame_index)
            .set_image_available_semaphore(image_available_semaphore);
        drop(acquire_span);

        let mut rendering_complete_command_buffer = rendering_complete_command_buffer.get_mut(&frame_index);

        // Render the frames
        frames.get_mut(&presenter_shared.frame_index).render_frame(
            &window_id,
            &backend_shared,
            &mut *presenter_shared,
            &immediate_rendering_frames,
            &mut rendering_complete_command_buffer,
        )?;

        // Present
        let rendering_complete_semaphore = frames
            .get(&frame_index)
            .rendering_complete_semaphore()
            .expect("rendering_complete_semaphore not set");
        let result = {
            // The queues must be dropped before `recreate` is called to prevent a deadlock.
            let mut queues = backend_shared.queue_scheduler.queues();
            presenter_shared.swapchain().present(
                &presenter_shared.frame_index,
                rendering_complete_semaphore,
                queues.presentation_queue(window_id),
            )
        };
        match result {
            Ok(is_suboptimal) => {
                if is_suboptimal {
                    info!("Swapchain is suboptimal. Recreating swapchain.");
                    presenter_shared.recreate(&window_id, &backend_shared)?;
                }
            }
            Err(_err) => {
                info!("Failed to present swapchain image. Recreating swapchain.");
                presenter_shared.recreate(&window_id, &backend_shared)?;
            }
        }
        drop(presenter_shared);

        loop_helper.loop_sleep();
    }
}
