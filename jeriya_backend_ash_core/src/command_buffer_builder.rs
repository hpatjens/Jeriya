use std::sync::Arc;

use ash::vk;

use crate::{
    buffer::VertexBuffer, command_buffer::CommandBuffer, device::Device, device_visible_buffer::DeviceVisibleBuffer,
    graphics_pipeline::GraphicsPipeline, host_visible_buffer::HostVisibleBuffer, simple_graphics_pipeline::SimpleGraphicsPipeline,
    swapchain::Swapchain, swapchain_depth_buffer::SwapchainDepthBuffer, swapchain_framebuffers::SwapchainFramebuffers,
    swapchain_render_pass::SwapchainRenderPass, AsRawVulkan,
};

pub struct CommandBufferBuilder<'buf> {
    command_buffer: &'buf mut CommandBuffer,
    device: Arc<Device>,
}

impl<'buf> CommandBufferBuilder<'buf> {
    pub fn new(device: &Arc<Device>, command_buffer: &'buf mut CommandBuffer) -> crate::Result<Self> {
        Ok(Self {
            command_buffer,
            device: device.clone(),
        })
    }
}

impl<'buf> CommandBufferBuilder<'buf> {
    pub fn begin_command_buffer(self) -> crate::Result<CommandBufferBuilder<'buf>> {
        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .as_raw_vulkan()
                .begin_command_buffer(*self.command_buffer.as_raw_vulkan(), &command_buffer_begin_info)?;
        }
        Ok(self)
    }

    pub fn end_command_buffer(self) -> crate::Result<()> {
        unsafe {
            self.device
                .as_raw_vulkan()
                .end_command_buffer(*self.command_buffer.as_raw_vulkan())?;
        }
        Ok(())
    }

    pub fn begin_render_pass(
        self,
        swapchain: &Swapchain,
        render_pass: &SwapchainRenderPass,
        framebuffer: (&SwapchainFramebuffers, usize),
    ) -> crate::Result<CommandBufferBuilder<'buf>> {
        let rect = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain.extent(),
        };

        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.6, 0.6, 0.9, 0.0],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
            },
        ];

        let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(render_pass.render_pass)
            .framebuffer(framebuffer.0.framebuffers[framebuffer.1])
            .render_area(rect)
            .clear_values(&clear_values);
        unsafe {
            self.device.as_raw_vulkan().cmd_begin_render_pass(
                *self.command_buffer.as_raw_vulkan(),
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );
        }
        Ok(self)
    }

    pub fn end_render_pass(self) -> crate::Result<CommandBufferBuilder<'buf>> {
        unsafe {
            self.device
                .as_raw_vulkan()
                .cmd_end_render_pass(*self.command_buffer.as_raw_vulkan());
        }
        Ok(self)
    }

    pub fn begin_command_buffer_for_one_time_submit(self) -> crate::Result<Self> {
        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .as_raw_vulkan()
                .begin_command_buffer(*self.command_buffer.as_raw_vulkan(), &command_buffer_begin_info)?;
        }
        Ok(self)
    }

    pub fn bind_graphics_pipeline(self, graphics_pipeline: &dyn GraphicsPipeline) -> Self {
        unsafe {
            self.device.as_raw_vulkan().cmd_bind_pipeline(
                *self.command_buffer.as_raw_vulkan(),
                vk::PipelineBindPoint::GRAPHICS,
                *graphics_pipeline.as_raw_vulkan(),
            );
        }
        self
    }

    pub fn bind_vertex_buffers<'arc, T>(self, first_binding: u32, vertex_buffer: impl Into<VertexBuffer<'arc, T>>) -> Self
    where
        T: Copy + 'static,
    {
        let vertex_buffer = vertex_buffer.into();
        unsafe {
            self.device.as_raw_vulkan().cmd_bind_vertex_buffers(
                *self.command_buffer.as_raw_vulkan(),
                first_binding,
                &[*vertex_buffer.as_raw_vulkan()],
                &[0],
            );
            self.command_buffer.push_dependency(vertex_buffer.as_command_buffer_dependency());
        }
        self
    }

    pub fn draw_three_vertices(self) -> Self {
        unsafe {
            self.device
                .as_raw_vulkan()
                .cmd_draw(*self.command_buffer.as_raw_vulkan(), 3, 1, 0, 0);
        }
        self
    }

    /// Draw vertices with the given `vertex_count` and `first_vertex`
    pub fn draw_vertices(self, vertex_count: u32, first_vertex: u32) -> Self {
        unsafe {
            self.device
                .as_raw_vulkan()
                .cmd_draw(*self.command_buffer.as_raw_vulkan(), vertex_count, 1, first_vertex, 0);
        }
        self
    }

    pub fn copy_buffer_from_host_to_device<T: Copy + 'static>(
        self,
        src: &Arc<HostVisibleBuffer<T>>,
        dst: &Arc<DeviceVisibleBuffer<T>>,
    ) -> Self {
        assert_eq!(src.byte_size(), dst.byte_size(), "buffers must have the same size");
        unsafe {
            let copy_region = vk::BufferCopy {
                src_offset: 0,
                dst_offset: 0,
                size: src.byte_size() as u64,
            };
            self.device.as_raw_vulkan().cmd_copy_buffer(
                *self.command_buffer.as_raw_vulkan(),
                *src.as_raw_vulkan(),
                *dst.as_raw_vulkan(),
                &[copy_region],
            );
            self.command_buffer.push_dependency(src.clone());
            self.command_buffer.push_dependency(dst.clone());
            self
        }
    }

    /// Special function for depth buffer layout transition
    pub fn depth_pipeline_barrier(self, swapchain_depth_buffer: &SwapchainDepthBuffer) -> crate::Result<Self> {
        let layout_transition_barriers = vk::ImageMemoryBarrier::builder()
            .image(swapchain_depth_buffer.depth_image)
            .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL)
                    .layer_count(1)
                    .level_count(1)
                    .build(),
            )
            .build();
        unsafe {
            self.device.as_raw_vulkan().cmd_pipeline_barrier(
                *self.command_buffer.as_raw_vulkan(),
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[layout_transition_barriers],
            )
        };
        Ok(self)
    }
}
