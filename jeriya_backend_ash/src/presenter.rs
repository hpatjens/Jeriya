use std::sync::Arc;

use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    device::Device, semaphore::Semaphore, surface::Surface, swapchain::Swapchain, swapchain_depth_buffer::SwapchainDepthBuffers,
    swapchain_framebuffers::SwapchainFramebuffers, swapchain_render_pass::SwapchainRenderPass, swapchain_vec::SwapchainVec,
};

/// All the state that is required for presenting to the [`Surface`]
pub struct Presenter {
    desired_swapchain_length: u32,
    pub image_available_semaphore: SwapchainVec<Semaphore>,
    pub rendering_complete_semaphore: SwapchainVec<Semaphore>,
    surface: Arc<Surface>,
    swapchain: Swapchain,
    swapchain_depth_buffers: SwapchainDepthBuffers,
    swapchain_framebuffers: SwapchainFramebuffers,
    swapchain_render_pass: SwapchainRenderPass,
    device: Arc<Device>,
}

impl Presenter {
    /// Creates a new `Presenter` for the [`Surface`]
    pub fn new(device: &Arc<Device>, surface: &Arc<Surface>, desired_swapchain_length: u32) -> core::Result<Self> {
        let swapchain = Swapchain::new(device, surface, desired_swapchain_length, None)?;
        let swapchain_depth_buffers = SwapchainDepthBuffers::new(device, &swapchain)?;
        let swapchain_render_pass = SwapchainRenderPass::new(device, &swapchain)?;
        let swapchain_framebuffers = SwapchainFramebuffers::new(device, &swapchain, &swapchain_depth_buffers, &swapchain_render_pass)?;
        let image_available_semaphore = SwapchainVec::new(&swapchain, |_| Semaphore::new(device))?;
        let rendering_complete_semaphore = SwapchainVec::new(&swapchain, |_| Semaphore::new(device))?;
        Ok(Self {
            desired_swapchain_length,
            image_available_semaphore,
            rendering_complete_semaphore,
            surface: surface.clone(),
            swapchain,
            swapchain_depth_buffers,
            swapchain_framebuffers,
            swapchain_render_pass,
            device: device.clone(),
        })
    }

    /// Creates the swapchain and all state that depends on it
    pub fn recreate(&mut self) -> core::Result<()> {
        self.device.wait_for_idle()?;
        self.swapchain = Swapchain::new(&self.device, &self.surface, self.desired_swapchain_length, Some(&self.swapchain))?;
        self.swapchain_depth_buffers = SwapchainDepthBuffers::new(&self.device, &self.swapchain)?;
        self.swapchain_render_pass = SwapchainRenderPass::new(&self.device, &self.swapchain)?;
        self.swapchain_framebuffers = SwapchainFramebuffers::new(
            &self.device,
            &self.swapchain,
            &self.swapchain_depth_buffers,
            &self.swapchain_render_pass,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::iter;

        use jeriya_test::create_window;

        use jeriya_backend_ash_core::{
            device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface,
        };

        use crate::presenter::Presenter;

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance, iter::once(&surface)).unwrap();
            let device = Device::new(physical_device, &instance).unwrap();
            let _presenter = Presenter::new(&device, &surface, 2).unwrap();
        }
    }
}
