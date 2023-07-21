use std::{
    sync::{
        mpsc::{channel, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    backend_shared::BackendShared,
    frame::{self, Frame},
    presenter_shared::{self, PresenterShared},
    ImmediateRenderingRequest,
};
use base::{
    command_buffer::CommandBuffer,
    command_pool::{CommandPool, CommandPoolCreateFlags},
    queue::{Queue, QueueType},
};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{semaphore::Semaphore, surface::Surface, swapchain_vec::SwapchainVec};
use jeriya_shared::{
    debug_info,
    log::error,
    parking_lot::Mutex,
    tracy_client::{span, Client},
    winit::window::WindowId,
    Handle,
};

const PRESENTER_NAMES: [&'static str; 8] = [
    "presenter_thread_0",
    "presenter_thread_1",
    "presenter_thread_2",
    "presenter_thread_3",
    "presenter_thread_4",
    "presenter_thread_5",
    "presenter_thread_6",
    "presenter_thread_unknown",
];

fn render_thread(
    window_id: &WindowId,
    presentation_queue: &mut Queue,
    presenter_shared: &mut PresenterShared,
    frames: &mut SwapchainVec<Frame>,
    backend_shared: &BackendShared,
    rendering_complete_command_buffer: &mut SwapchainVec<Option<Arc<CommandBuffer>>>,
) -> jeriya_shared::Result<()> {
    // Finish command buffer execution
    presentation_queue.poll_completed_fences()?;

    // Acquire the next swapchain index and set the frame index
    let prepare_span = span!("prepare next image");
    let image_available_semaphore = Arc::new(Semaphore::new(&backend_shared.device, debug_info!("image-available-Semaphore"))?);
    let frame_index = presenter_shared
        .swapchain()
        .acquire_next_image(&presenter_shared.frame_index, &image_available_semaphore)?;
    presenter_shared.frame_index = frame_index.clone();
    frames
        .get_mut(&presenter_shared.frame_index)
        .set_image_available_semaphore(image_available_semaphore);
    drop(prepare_span);

    let mut rendering_complete_command_buffer = rendering_complete_command_buffer.get_mut(&frame_index);

    // Render the frames
    frames.get_mut(&presenter_shared.frame_index).render_frame(
        window_id,
        presentation_queue,
        backend_shared,
        &mut *presenter_shared,
        rendering_complete_command_buffer,
    )?;

    // Present
    let present_span = span!("present");
    presenter_shared.swapchain().present(
        &presenter_shared.frame_index,
        frames.get(&frame_index).rendering_complete_semaphores(),
        presentation_queue,
    )?;
    drop(present_span);

    Ok(())
}

pub struct Presenter {
    presenter_index: usize,
    thread: JoinHandle<()>,
    sender: Sender<()>,
    presenter_shared: Arc<Mutex<PresenterShared>>,
    frames: Arc<Mutex<SwapchainVec<Frame>>>,
}

impl Presenter {
    pub fn new(
        presenter_index: usize,
        window_id: &WindowId,
        surface: &Arc<Surface>,
        backend_shared: Arc<BackendShared>,
    ) -> jeriya_shared::Result<Self> {
        let presenter_shared = Arc::new(Mutex::new(PresenterShared::new(window_id, &backend_shared, surface)?));
        let frames = Arc::new(Mutex::new(SwapchainVec::new(presenter_shared.lock().swapchain(), |_| {
            Frame::new(window_id, &backend_shared)
        })?));
        let (sender, receiver) = channel();
        let window_id = window_id.clone();
        let presenter_shared2 = presenter_shared.clone();
        let frames2 = frames.clone();
        let thread = thread::spawn(move || {
            let name = PRESENTER_NAMES[presenter_index.min(PRESENTER_NAMES.len() - 1)];
            let client = Client::start();
            client.set_thread_name(name);

            let Ok(mut presentation_queue) = Queue::new(&backend_shared.device, QueueType::Presentation, presenter_index as u32 + 1) else {
                panic!("Failed to allocate presentation Queue for Presenter {presenter_index} (Window: {window_id:?})");
            };

            let Ok(mut rendering_complete_command_buffer) = SwapchainVec::new(&presenter_shared2.lock().swapchain(), |_| Ok(None)) else {
                panic!("Failed to create SwapchainVec for rendering complete CommandBuffers for Presenter {presenter_index} (Window: {window_id:?})");
            };

            loop {
                if let Err(_) = receiver.recv() {
                    panic!("Failed to receive message for Presenter {presenter_index} (Window: {window_id:?})");
                }
                let mut presenter_shared = presenter_shared2.lock();
                let mut frames = frames2.lock();
                render_thread(
                    &window_id,
                    &mut presentation_queue,
                    &mut *presenter_shared,
                    &mut *frames,
                    &backend_shared,
                    &mut rendering_complete_command_buffer,
                )
                .unwrap();
            }
        });
        Ok(Self {
            presenter_index,
            thread,
            sender,
            presenter_shared,
            frames,
        })
    }

    pub fn request_frame(&mut self) -> jeriya_shared::Result<()> {
        let _span = span!("Presenter::request_frame");
        self.sender.send(());
        Ok(())
    }

    /// Enqueues an [`ImmediateRenderingRequest`]
    pub fn render_immediate_command_buffer(&self, immediate_rendering_request: ImmediateRenderingRequest) {
        self.presenter_shared
            .lock()
            .immediate_rendering_requests
            .push(immediate_rendering_request);
    }

    /// Recreates the [`PresenterShared`] in case of a swapchain resize
    pub fn recreate(&self) -> base::Result<()> {
        self.presenter_shared.lock().recreate()
    }

    /// Sets the active camera
    pub fn set_active_camera(&self, handle: Handle<jeriya_shared::Camera>) {
        self.presenter_shared.lock().active_camera = handle;
    }

    /// Returns the active camera
    pub fn active_camera(&self) -> Handle<jeriya_shared::Camera> {
        self.presenter_shared.lock().active_camera.clone()
    }
}
