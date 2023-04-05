use std::sync::Arc;

use ash::vk;

use crate::{
    device::Device, swapchain::Swapchain, swapchain_depth_buffer::SwapchainDepthBuffers, swapchain_render_pass::SwapchainRenderPass,
    AsRawVulkan,
};

/// Framebuffers for the Swapchain
pub struct SwapchainFramebuffers {
    pub framebuffers: Vec<vk::Framebuffer>,
    device: Arc<Device>,
}

impl Drop for SwapchainFramebuffers {
    fn drop(&mut self) {
        for framebuffer in &self.framebuffers {
            unsafe { self.device.as_raw_vulkan().destroy_framebuffer(*framebuffer, None) };
        }
    }
}

impl SwapchainFramebuffers {
    /// Creates a new `SwapchainFramebuffers` for the given [`Swapchain`]
    pub fn new(
        device: &Arc<Device>,
        swapchain: &Swapchain,
        swapchain_depth_buffers: &SwapchainDepthBuffers,
        swapchain_render_pass: &SwapchainRenderPass,
    ) -> crate::Result<Self> {
        let framebuffers = swapchain
            .image_views()
            .iter()
            .zip(swapchain_depth_buffers.depth_buffers.iter())
            .map(|(present_image_view, depth_buffer)| {
                let framebuffer_attachments = [*present_image_view, depth_buffer.depth_image_view];
                let frame_buffer_create_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(*swapchain_render_pass.as_raw_vulkan())
                    .attachments(&framebuffer_attachments)
                    .width(swapchain.extent().width)
                    .height(swapchain.extent().height)
                    .layers(1);
                unsafe { device.as_raw_vulkan().create_framebuffer(&frame_buffer_create_info, None) }
            })
            .collect::<Result<Vec<vk::Framebuffer>, _>>()?;
        Ok(Self {
            framebuffers,
            device: device.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::iter;

        use jeriya_test::create_window;

        use crate::{
            device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface, swapchain::Swapchain,
            swapchain_depth_buffer::SwapchainDepthBuffers, swapchain_framebuffers::SwapchainFramebuffers,
            swapchain_render_pass::SwapchainRenderPass,
        };

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance, iter::once(&surface)).unwrap();
            let device = Device::new(physical_device, &instance).unwrap();
            let swapchain = Swapchain::new(&instance, &device, &surface).unwrap();
            let swapchain_depth_buffer = SwapchainDepthBuffers::new(&device, &swapchain).unwrap();
            let swapchain_render_pass = SwapchainRenderPass::new(&device, &swapchain).unwrap();
            let _swapchain_framebuffers =
                SwapchainFramebuffers::new(&device, &swapchain, &swapchain_depth_buffer, &swapchain_render_pass).unwrap();
        }
    }
}
