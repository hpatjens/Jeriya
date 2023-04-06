use std::sync::Arc;

use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{device::Device, surface::Surface};

use crate::presenter_resources::PresenterResources;

pub struct Presenter {
    presenter_resources: PresenterResources,
}

impl Presenter {
    pub fn new(device: &Arc<Device>, surface: &Arc<Surface>, desired_swapchain_length: u32) -> core::Result<Self> {
        Ok(Self {
            presenter_resources: PresenterResources::new(device, surface, desired_swapchain_length)?,
        })
    }

    /// Recreates the [`PresenterResources`] in case of a swapchain resize
    pub fn recreate(&mut self) -> core::Result<()> {
        self.presenter_resources.recreate()
    }
}
