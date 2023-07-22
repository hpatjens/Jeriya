use crate::{backend_shared::BackendShared, frame::Frame, presenter_shared::PresenterShared};
use jeriya_backend_ash_base::{
    command_buffer::CommandBuffer,
    queue::{Queue, QueueType},
    semaphore::Semaphore,
    swapchain_vec::SwapchainVec,
};
use jeriya_shared::{
    self,
    crossbeam_channel::{bounded, Sender},
    debug_info,
    parking_lot::Mutex,
    tracy_client::{span, Client},
    winit::window::WindowId,
};
use std::{
    sync::Arc,
    thread::{self, JoinHandle},
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

pub struct PresenterThread {
    _presenter_index: usize,
    _thread: JoinHandle<()>,
    frame_request_sender: Sender<()>,
}

impl PresenterThread {
    /// Spawns a new presenter thread that will render frames that are requested via [`PresenterThread::request_frame`].
    pub fn spawn(
        presenter_index: usize,
        window_id: WindowId,
        backend_shared: Arc<BackendShared>,
        presenter_shared: Arc<Mutex<PresenterShared>>,
        frames: Arc<Mutex<SwapchainVec<Frame>>>,
    ) -> jeriya_shared::Result<Self> {
        // Channel for requesting a frame from the renderer thread. Requesting more frames than the swapchain can hold will block the thread.
        let (frame_request_sender, frame_request_receiver) = bounded(presenter_shared.lock().swapchain().len());

        let thread = thread::spawn(move || {
            // Setup Tracy profiling
            let name = PRESENTER_NAMES[presenter_index.min(PRESENTER_NAMES.len() - 1)];
            let client = Client::start();
            client.set_thread_name(name);

            // Thread-local Queue for the Presenter
            let Ok(mut presentation_queue) = Queue::new(&backend_shared.device, QueueType::Presentation, presenter_index as u32 + 1) else {
                panic!("Failed to allocate presentation Queue for Presenter {presenter_index} (Window: {window_id:?})");
            };

            // Command Buffer that is checked to determine whether the rendering is complete
            let Ok(mut rendering_complete_command_buffer) = SwapchainVec::new(&presenter_shared.lock().swapchain(), |_| Ok(None)) else {
                panic!("Failed to create SwapchainVec for rendering complete CommandBuffers for Presenter {presenter_index} (Window: {window_id:?})");
            };

            loop {
                if let Err(_) = frame_request_receiver.recv() {
                    panic!("Failed to receive message for Presenter {presenter_index} (Window: {window_id:?})");
                }
                let mut presenter_shared = presenter_shared.lock();
                let mut frames = frames.lock();

                // Render the frame
                if let Err(err) = render(
                    &window_id,
                    &mut presentation_queue,
                    &mut *presenter_shared,
                    &mut *frames,
                    &backend_shared,
                    &mut rendering_complete_command_buffer,
                ) {
                    panic!("Failed to render frame for Presenter {presenter_index} (Window: {window_id:?}): {err:?}");
                }
            }
        });

        Ok(Self {
            _presenter_index: presenter_index,
            _thread: thread,
            frame_request_sender,
        })
    }

    /// Sends a request to the presenter thread to render a frame.
    ///
    /// This will block when more frames are requested than the swapchain can hold.
    pub fn request_frame(&mut self) -> jeriya_shared::Result<()> {
        let _span = span!("PresenterThread::request_frame");
        // Just shutting down the whole renderer when one of the presenter threads has shut down unexpectedly.
        self.frame_request_sender
            .send(())
            .expect("Failed to send message to Presenter thread");
        Ok(())
    }
}

fn render(
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

    let rendering_complete_command_buffer = rendering_complete_command_buffer.get_mut(&frame_index);

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
