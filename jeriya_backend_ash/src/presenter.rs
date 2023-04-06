use std::sync::Arc;

use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{device::Device, semaphore::Semaphore, surface::Surface, swapchain_vec::SwapchainVec};

use crate::presenter_resources::PresenterResources;

pub struct Presenter {
    presenter_resources: PresenterResources,
    pub image_available_semaphore: SwapchainVec<Semaphore>,
    pub rendering_complete_semaphore: SwapchainVec<Semaphore>,
}

impl Presenter {
    pub fn new(device: &Arc<Device>, surface: &Arc<Surface>, desired_swapchain_length: u32) -> core::Result<Self> {
        let presenter_resources = PresenterResources::new(device, surface, desired_swapchain_length)?;
        let image_available_semaphore = SwapchainVec::new(presenter_resources.swapchain(), |_| Semaphore::new(device))?;
        let rendering_complete_semaphore = SwapchainVec::new(presenter_resources.swapchain(), |_| Semaphore::new(device))?;
        Ok(Self {
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
