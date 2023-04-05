use std::sync::Arc;

use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    device::Device, instance::Instance, surface::Surface, swapchain::Swapchain, swapchain_depth_buffer::SwapchainDepthBuffer,
    swapchain_depth_buffer::SwapchainDepthBuffers, swapchain_framebuffers::SwapchainFramebuffers,
    swapchain_render_pass::SwapchainRenderPass,
};

/// All the state that is required for presenting to the [`Surface`]
pub struct Presenter {
    _swapchain: Swapchain,
    _swapchain_depth_buffers: SwapchainDepthBuffers,
    _swapchain_framebuffers: SwapchainFramebuffers,
    _swapchain_render_pass: SwapchainRenderPass,
}

impl Presenter {
    /// Creates a new `Presenter` for the [`Surface`]
    pub fn new(instance: &Arc<Instance>, device: &Arc<Device>, surface: &Arc<Surface>) -> core::Result<Self> {
        let swapchain = Swapchain::new(instance, device, surface)?;
        let swapchain_depth_buffers = SwapchainDepthBuffers::new(device, &swapchain)?;
        let swapchain_render_pass = SwapchainRenderPass::new(device, &swapchain)?;
        let swapchain_framebuffers = SwapchainFramebuffers::new(device, &swapchain, &swapchain_depth_buffers, &swapchain_render_pass)?;
        Ok(Self {
            _swapchain: swapchain,
            _swapchain_depth_buffers: swapchain_depth_buffers,
            _swapchain_framebuffers: swapchain_framebuffers,
            _swapchain_render_pass: swapchain_render_pass,
        })
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
            let _presenter = Presenter::new(&instance, &device, &surface).unwrap();
        }
    }
}
