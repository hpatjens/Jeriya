use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::{backend_shared::BackendShared, frame::Frame, presenter_shared::PresenterShared, ImmediateRenderingRequest};
use base::{
    queue::{Queue, QueueType},
    semaphore::Semaphore,
};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{surface::Surface, swapchain_vec::SwapchainVec};
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

#[derive(Debug, Clone)]
pub enum PresenterEvent {
    Recreate,
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
    fn send(&self, event: PresenterEvent) {
        self.event_queue.lock().push(event);
    }

    /// Returns the index of the presenter
    pub fn presenter_index(&self) -> usize {
        self._presenter_index
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

        trace!("presenter {presenter_index}: thread loop start (framerate: {frame_rate:?})");

        let mut presenter_shared = presenter_shared.lock();

        let mut event_queue = event_queue.lock().take();
        while let Some(new_events) = event_queue.pop() {
            match new_events {
                PresenterEvent::Recreate => {
                    presenter_shared.recreate(&presentation_queue)?;
                }
            }
        }

        trace!("presenter {presenter_index}: locked presenter_shared");

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
        let rendering_complete_semaphore = frames
            .get(&frame_index)
            .rendering_complete_semaphore()
            .expect("rendering_complete_semaphore not set");
        presenter_shared
            .swapchain()
            .present(&presenter_shared.frame_index, rendering_complete_semaphore, &presentation_queue)?;

        drop(presenter_shared);

        loop_helper.loop_sleep();
    }
}
