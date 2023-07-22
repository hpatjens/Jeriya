use std::sync::Arc;

use crate::{
    backend_shared::BackendShared, frame::Frame, presenter_shared::PresenterShared, presenter_thread::PresenterThread,
    ImmediateRenderingRequest,
};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{surface::Surface, swapchain_vec::SwapchainVec};
use jeriya_shared::{parking_lot::Mutex, tracy_client::span, winit::window::WindowId, Handle};

pub struct Presenter {
    _presenter_index: usize,
    thread: PresenterThread,
    presenter_shared: Arc<Mutex<PresenterShared>>,
    _frames: Arc<Mutex<SwapchainVec<Frame>>>,
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
            Frame::new(presenter_index, window_id, &backend_shared)
        })?));

        // Spawn the presenter thread
        let thread = PresenterThread::spawn(
            presenter_index,
            window_id.clone(),
            backend_shared,
            presenter_shared.clone(),
            frames.clone(),
        )?;

        Ok(Self {
            _presenter_index: presenter_index,
            thread,
            presenter_shared,
            _frames: frames,
        })
    }

    /// Returns the index of the presenter
    pub fn presenter_index(&self) -> usize {
        self._presenter_index
    }

    /// Sends a request to the presenter thread to render a frame.
    ///
    /// This will block when more frames are requested than the swapchain can hold.
    pub fn request_frame(&mut self) -> jeriya_shared::Result<()> {
        let _span = span!("Presenter::request_frame");
        self.thread.request_frame()?;
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
