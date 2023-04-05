use std::cell::RefCell;
use std::sync::Arc;

use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    device::Device, surface::Surface, swapchain::Swapchain, swapchain_depth_buffer::SwapchainDepthBuffers,
    swapchain_framebuffers::SwapchainFramebuffers, swapchain_render_pass::SwapchainRenderPass,
};

/// All the state that is required for presenting to the [`Surface`]
pub struct Presenter {
    inner: RefCell<Inner>,
}

impl Presenter {
    /// Creates a new `Presenter` for the [`Surface`]
    pub fn new(device: &Arc<Device>, surface: &Arc<Surface>) -> core::Result<Self> {
        let inner = Inner::new(device, surface)?;
        Ok(Self {
            inner: RefCell::new(inner),
        })
    }

    /// Creates the swapchain and all state that depends on it
    pub fn recreate(&self) -> core::Result<()> {
        self.inner.borrow_mut().recreate()
    }
}

struct Inner {
    swapchain: Swapchain,
    swapchain_depth_buffers: SwapchainDepthBuffers,
    swapchain_framebuffers: SwapchainFramebuffers,
    swapchain_render_pass: SwapchainRenderPass,
    device: Arc<Device>,
}

impl Inner {
    pub fn new(device: &Arc<Device>, surface: &Arc<Surface>) -> core::Result<Self> {
        let swapchain = Swapchain::new(device.instance(), device, surface)?;
        let swapchain_depth_buffers = SwapchainDepthBuffers::new(device, &swapchain)?;
        let swapchain_render_pass = SwapchainRenderPass::new(device, &swapchain)?;
        let swapchain_framebuffers = SwapchainFramebuffers::new(device, &swapchain, &swapchain_depth_buffers, &swapchain_render_pass)?;
        Ok(Self {
            swapchain,
            swapchain_depth_buffers,
            swapchain_framebuffers,
            swapchain_render_pass,
            device: device.clone(),
        })
    }

    pub fn recreate(&mut self) -> core::Result<()> {
        self.device.wait_for_idle()?;
        self.swapchain.recreate()?;
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
            let _presenter = Presenter::new(&device, &surface).unwrap();
        }
    }
}
