use ash::vk;

use std::sync::Arc;

use crate::{device::Device, swapchain::Swapchain, AsRawVulkan};

pub struct SwapchainRenderPass {
    pub render_pass: vk::RenderPass,
    device: Arc<Device>,
}

impl Drop for SwapchainRenderPass {
    fn drop(&mut self) {
        unsafe {
            self.device.as_raw_vulkan().destroy_render_pass(self.render_pass, None);
        }
    }
}

impl SwapchainRenderPass {
    pub fn new(device: &Arc<Device>, swapchain: &Swapchain) -> crate::Result<Self> {
        let renderpass_attachments = [
            vk::AttachmentDescription {
                format: swapchain.surface_format().format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                initial_layout: vk::ImageLayout::UNDEFINED,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
            vk::AttachmentDescription {
                format: vk::Format::D24_UNORM_S8_UINT,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
        ];
        let color_attachment_refs = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
        let depth_attachment_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };
        let dependencies = [vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ..Default::default()
        }];

        let subpasses = [vk::SubpassDescription::builder()
            .color_attachments(&color_attachment_refs)
            .depth_stencil_attachment(&depth_attachment_ref)
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .build()];

        let renderpass_create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&renderpass_attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        let render_pass = unsafe { device.as_raw_vulkan().create_render_pass(&renderpass_create_info, None)? };

        Ok(Self {
            render_pass,
            device: device.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use jeriya_test::create_window;

    use crate::{
        device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface, swapchain::Swapchain,
    };

    use super::SwapchainRenderPass;

    #[test]
    fn smoke() {
        let window = create_window();
        let entry = Entry::new().unwrap();
        let instance = Instance::new(&entry, "my_application", false).unwrap();
        let surface = Surface::new(&entry, &instance, &window).unwrap();
        let physical_device = PhysicalDevice::new(&instance, iter::once(&surface)).unwrap();
        let device = Device::new(physical_device, &instance).unwrap();
        let swapchain = Swapchain::new(&instance, &device, &surface).unwrap();
        let _swapchain_renderpass = SwapchainRenderPass::new(&device, &swapchain).unwrap();
    }
}