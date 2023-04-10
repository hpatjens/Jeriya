use std::sync::Arc;

use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    device::Device, frame_index::FrameIndex, semaphore::Semaphore, surface::Surface, swapchain_vec::SwapchainVec,
};

use crate::presenter_resources::PresenterResources;

pub struct Presenter {
    pub frame_index: FrameIndex,
    pub presenter_resources: PresenterResources,
    pub image_available_semaphore: SwapchainVec<Option<Semaphore>>,
    pub rendering_complete_semaphore: SwapchainVec<Semaphore>,
}

impl Presenter {
    pub fn new(device: &Arc<Device>, surface: &Arc<Surface>, desired_swapchain_length: u32) -> core::Result<Self> {
        let presenter_resources = PresenterResources::new(device, surface, desired_swapchain_length)?;
        let image_available_semaphore = SwapchainVec::new(presenter_resources.swapchain(), |_| Ok(None))?;
        let rendering_complete_semaphore = SwapchainVec::new(presenter_resources.swapchain(), |_| Semaphore::new(device))?;
        let frame_index = FrameIndex::new();
        Ok(Self {
            frame_index,
            presenter_resources,
            image_available_semaphore,
            rendering_complete_semaphore,
        })
    }

    /// Recreates the [`PresenterResources`] in case of a swapchain resize
    pub fn recreate(&mut self) -> core::Result<()> {
        self.presenter_resources.recreate()
    }
}
