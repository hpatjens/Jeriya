use crate::{backend_shared::BackendShared, frame::Frame, presenter_shared::PresenterShared};
use jeriya_backend_ash_base::{
    queue::{Queue, QueueType},
    semaphore::Semaphore,
    swapchain_vec::SwapchainVec,
};
use jeriya_macros::profile;
use jeriya_shared::{
    self, debug_info,
    log::info,
    parking_lot::Mutex,
    spin_sleep,
    tracy_client::{span, Client},
    winit::window::WindowId,
    AsDebugInfo, FrameRate,
};
use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

pub struct PresenterThread {
    _presenter_index: usize,
    _thread: JoinHandle<()>,
}

#[profile]
impl PresenterThread {
    /// Spawns a new presenter thread that will render frames that are requested via [`PresenterThread::request_frame`].
    pub fn spawn(
        presenter_index: usize,
        window_id: WindowId,
        backend_shared: Arc<BackendShared>,
        presenter_shared: Arc<Mutex<PresenterShared>>,
        frame_rate: FrameRate,
    ) -> jeriya_backend::Result<Self> {
        let thread = thread::spawn(move || {
            if let Err(err) = run_presenter_thread(presenter_index, backend_shared, presenter_shared, window_id, frame_rate) {
                panic!("Error on PresenterThread {presenter_index} (Window: {window_id:?}): {err:?}");
            }
        });

        Ok(Self {
            _presenter_index: presenter_index,
            _thread: thread,
        })
    }
}

fn run_presenter_thread(
    presenter_index: usize,
    backend_shared: Arc<BackendShared>,
    presenter_shared: Arc<Mutex<PresenterShared>>,
    window_id: WindowId,
    frame_rate: FrameRate,
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

    // Thread-local Queue for the Presenter
    let mut presentation_queue = Queue::new(
        &backend_shared.device,
        QueueType::Presentation,
        presenter_index as u32 + 1, // +1 because the first queue is used by the backend for transfering resources
        debug_info!(format!("presenter-thread-queue-{}", presenter_index)),
    )?;

    // Command Buffer that is checked to determine whether the rendering is complete
    let mut rendering_complete_command_buffer = SwapchainVec::new(&presenter_shared.lock().swapchain(), |_| Ok(None))?;

    let mut loop_helper = match frame_rate {
        FrameRate::Unlimited => spin_sleep::LoopHelper::builder().build_without_target_rate(),
        FrameRate::Limited(frame_rate) => spin_sleep::LoopHelper::builder().build_with_target_rate(frame_rate as f64),
    };

    info!("Starting presenter loop with frame rate: {:?}", frame_rate);
    loop {
        loop_helper.loop_start();

        println!("presenter {presenter_index} thread loop start (framerate: {frame_rate:?})");

        let mut presenter_shared = presenter_shared.lock();

        // Finish command buffer execution
        presentation_queue.poll_completed_fences()?;

        // Acquire the next swapchain image
        let acquire_span = span!("acquire swapchain image");
        let image_available_semaphore = Arc::new(Semaphore::new(&backend_shared.device, debug_info!("image-available-Semaphore"))?);
        let frame_index = presenter_shared
            .swapchain()
            .acquire_next_image(&presenter_shared.frame_index, &image_available_semaphore)?;
        presenter_shared.frame_index = frame_index.clone();
        frames
            .get_mut(&presenter_shared.frame_index)
            .set_image_available_semaphore(image_available_semaphore);
        drop(acquire_span);

        let mut rendering_complete_command_buffer = rendering_complete_command_buffer.get_mut(&frame_index);

        // Render the frames
        frames.get_mut(&presenter_shared.frame_index).render_frame(
            &window_id,
            &mut presentation_queue,
            &backend_shared,
            &mut *presenter_shared,
            &mut rendering_complete_command_buffer,
        )?;

        // Present
        presenter_shared.swapchain().present(
            &presenter_shared.frame_index,
            frames.get(&frame_index).rendering_complete_semaphores(),
            &presentation_queue,
        )?;

        drop(presenter_shared);

        loop_helper.loop_sleep();
    }
}
