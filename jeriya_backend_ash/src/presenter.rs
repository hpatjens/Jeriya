use std::{collections::VecDeque, sync::Arc};

use crate::{backend_shared::BackendShared, frame::Frame, presenter_shared::PresenterShared, ImmediateRenderingRequest};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{frame_index::FrameIndex, semaphore::Semaphore, surface::Surface, swapchain_vec::SwapchainVec};
use jeriya_shared::tracy_client::span;
use jeriya_shared::{debug_info, winit::window::WindowId, Handle};

pub struct Presenter {
    frame_index: FrameIndex,
    frame_index_history: VecDeque<FrameIndex>,
    presenter_shared: PresenterShared,
    frames: SwapchainVec<Frame>,
}

impl Presenter {
    pub fn new(window_id: &WindowId, surface: &Arc<Surface>, backend_shared: &BackendShared) -> jeriya_shared::Result<Self> {
        let presenter_shared = PresenterShared::new(window_id, backend_shared, surface)?;
        let frames = SwapchainVec::new(presenter_shared.swapchain(), |_| Frame::new(window_id, backend_shared))?;
        Ok(Self {
            frame_index: FrameIndex::new(),
            presenter_shared,
            frame_index_history: VecDeque::new(),
            frames,
        })
    }

    pub fn render_frame(&mut self, window_id: &WindowId, backend_shared: &BackendShared) -> jeriya_shared::Result<()> {
        let _span = span!("Presenter::render_frame");

        // Acquire the next swapchain index and set the frame index
        let image_available_semaphore = Arc::new(Semaphore::new(&backend_shared.device, debug_info!("image-available-Semaphore"))?);
        let frame_index = self
            .presenter_shared
            .swapchain()
            .acquire_next_image(&self.frame_index, &image_available_semaphore)?;
        self.start_frame(frame_index.clone());
        self.frames
            .get_mut(&self.frame_index())
            .set_image_available_semaphore(image_available_semaphore);

        // Render the frames
        self.frames
            .get_mut(&self.frame_index)
            .render_frame(&self.frame_index, window_id, backend_shared, &mut self.presenter_shared)?;

        // Present
        self.presenter_shared.swapchain().present(
            &self.frame_index(),
            self.frames.get(&frame_index).rendering_complete_semaphores(),
            &backend_shared.presentation_queue.borrow(),
        )?;

        Ok(())
    }

    /// Enqueues an [`ImmediateRenderingRequest`]
    pub fn render_immediate_command_buffer(&mut self, immediate_rendering_request: ImmediateRenderingRequest) {
        self.presenter_shared.immediate_rendering_requests.push(immediate_rendering_request);
    }

    /// Recreates the [`PresenterShared`] in case of a swapchain resize
    pub fn recreate(&mut self) -> base::Result<()> {
        self.presenter_shared.recreate()
    }

    /// Sets the given [`FrameIndex`] and appends the previous one to the history
    pub fn start_frame(&mut self, frame_index: FrameIndex) {
        self.frame_index_history.push_front(self.frame_index.clone());
        self.frame_index = frame_index;
        while self.frame_index_history.len() > self.presenter_shared.swapchain().len() - 1 {
            self.frame_index_history.pop_back();
        }
    }

    /// Returns the current [`FrameIndex`]
    pub fn frame_index(&self) -> FrameIndex {
        self.frame_index.clone()
    }

    /// Sets the active camera
    pub fn set_active_camera(&mut self, handle: Handle<jeriya_shared::Camera>) {
        self.presenter_shared.active_camera = handle;
    }

    /// Returns the active camera
    pub fn active_camera(&self) -> Handle<jeriya_shared::Camera> {
        self.presenter_shared.active_camera.clone()
    }

    /// Returns the [`FrameIndex`] of the oldest frame in the history
    #[allow(dead_code)]
    pub fn oldest_frame_index(&self) -> Option<FrameIndex> {
        self.frame_index_history.back().cloned()
    }
}
