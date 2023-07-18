use std::{mem, sync::Arc};

use ash::vk;
use jeriya_shared::parking_lot::Mutex;

use crate::{
    buffer::{Buffer, VertexBuffer},
    command_buffer::CommandBuffer,
    compute_pipeline::ComputePipeline,
    device::Device,
    device_visible_buffer::DeviceVisibleBuffer,
    graphics_pipeline::GraphicsPipeline,
    host_visible_buffer::HostVisibleBuffer,
    push_descriptors::PushDescriptors,
    swapchain::Swapchain,
    swapchain_depth_buffer::SwapchainDepthBuffer,
    swapchain_framebuffers::SwapchainFramebuffers,
    swapchain_render_pass::SwapchainRenderPass,
    AsRawVulkan, DrawIndirectCommand, Error,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineBindPoint {
    Graphics,
    Compute,
}

impl From<PipelineBindPoint> for vk::PipelineBindPoint {
    fn from(value: PipelineBindPoint) -> Self {
        match value {
            PipelineBindPoint::Graphics => vk::PipelineBindPoint::GRAPHICS,
            PipelineBindPoint::Compute => vk::PipelineBindPoint::COMPUTE,
        }
    }
}

pub struct CommandBufferBuilder<'buf> {
    command_buffer: &'buf mut CommandBuffer,
    device: Arc<Device>,

    /// Layout of the last pipeline that was bound if any
    bound_pipeline_layout: Option<vk::PipelineLayout>,
}

impl<'buf> CommandBufferBuilder<'buf> {
    pub fn new(device: &Arc<Device>, command_buffer: &'buf mut CommandBuffer) -> crate::Result<Self> {
        Ok(Self {
            command_buffer,
            device: device.clone(),
            bound_pipeline_layout: None,
        })
    }
}

impl<'buf> CommandBufferBuilder<'buf> {
    pub(crate) fn command_buffer(&mut self) -> &mut CommandBuffer {
        self.command_buffer
    }

    pub fn begin_command_buffer(&mut self) -> crate::Result<&mut Self> {
        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .as_raw_vulkan()
                .begin_command_buffer(*self.command_buffer.as_raw_vulkan(), &command_buffer_begin_info)?;
        }
        Ok(self)
    }

    pub fn end_command_buffer(&mut self) -> crate::Result<()> {
        unsafe {
            self.device
                .as_raw_vulkan()
                .end_command_buffer(*self.command_buffer.as_raw_vulkan())?;
        }
        Ok(())
    }

    pub fn begin_render_pass(
        &mut self,
        swapchain: &Swapchain,
        render_pass: &SwapchainRenderPass,
        framebuffer: (&SwapchainFramebuffers, usize),
    ) -> crate::Result<&mut Self> {
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

    pub fn end_render_pass(&mut self) -> crate::Result<&mut Self> {
        unsafe {
            self.device
                .as_raw_vulkan()
                .cmd_end_render_pass(*self.command_buffer.as_raw_vulkan());
        }
        Ok(self)
    }

    pub fn begin_command_buffer_for_one_time_submit(&mut self) -> crate::Result<&mut Self> {
        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .as_raw_vulkan()
                .begin_command_buffer(*self.command_buffer.as_raw_vulkan(), &command_buffer_begin_info)?;
        }
        Ok(self)
    }

    pub fn bind_graphics_pipeline(&mut self, graphics_pipeline: &dyn GraphicsPipeline) -> &mut Self {
        unsafe {
            self.device.as_raw_vulkan().cmd_bind_pipeline(
                *self.command_buffer.as_raw_vulkan(),
                vk::PipelineBindPoint::GRAPHICS,
                graphics_pipeline.graphics_pipeline(),
            );
            self.bound_pipeline_layout = Some(graphics_pipeline.graphics_pipeline_layout());
        }
        self
    }

    pub fn bind_compute_pipeline(&mut self, compute_pipeline: &dyn ComputePipeline) -> &mut Self {
        unsafe {
            self.device.as_raw_vulkan().cmd_bind_pipeline(
                *self.command_buffer.as_raw_vulkan(),
                vk::PipelineBindPoint::COMPUTE,
                compute_pipeline.compute_pipeline(),
            );
            self.bound_pipeline_layout = Some(compute_pipeline.compute_pipeline_layout());
        }
        self
    }

    pub fn bind_vertex_buffers<'arc, T>(&mut self, first_binding: u32, vertex_buffer: impl Into<VertexBuffer<'arc, T>>) -> &mut Self
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

    pub fn draw_three_vertices(&mut self) -> &mut Self {
        unsafe {
            self.device
                .as_raw_vulkan()
                .cmd_draw(*self.command_buffer.as_raw_vulkan(), 3, 1, 0, 0);
        }
        self
    }

    /// Draw vertices with the given `vertex_count` and `first_vertex`
    pub fn draw_vertices(&mut self, vertex_count: u32, first_vertex: u32) -> &mut Self {
        unsafe {
            self.device
                .as_raw_vulkan()
                .cmd_draw(*self.command_buffer.as_raw_vulkan(), vertex_count, 1, first_vertex, 0);
        }
        self
    }

    pub fn copy_buffer_from_host_to_device<T: Clone + 'static>(
        &mut self,
        src: &Arc<HostVisibleBuffer<T>>,
        dst: &Arc<DeviceVisibleBuffer<T>>,
    ) -> &mut Self {
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

    pub fn copy_buffer_range_from_device_to_host<T: Clone + 'static>(
        &mut self,
        src: &Arc<DeviceVisibleBuffer<T>>,
        byte_size: usize,
        dst: &Arc<Mutex<HostVisibleBuffer<T>>>,
    ) -> &mut Self {
        let dst_guard = dst.lock();
        assert!(
            byte_size <= src.byte_size(),
            "can't copy more bytes than the source buffer contains"
        );
        assert!(
            byte_size <= dst_guard.byte_size(),
            "can't copy more bytes than the destination buffer contains"
        );
        unsafe {
            let copy_region = vk::BufferCopy {
                src_offset: 0,
                dst_offset: 0,
                size: byte_size as u64,
            };
            self.device.as_raw_vulkan().cmd_copy_buffer(
                *self.command_buffer.as_raw_vulkan(),
                *src.as_raw_vulkan(),
                *dst_guard.as_raw_vulkan(),
                &[copy_region],
            );
            self.command_buffer.push_dependency(src.clone());
            self.command_buffer.push_dependency(dst.clone());
            self
        }
    }

    /// Pushes a closure to the list of operations to be executed when the command buffer has finished executing.
    pub fn push_finished_operation(&mut self, finished_operation: Box<dyn Fn() -> crate::Result<()> + 'static>) -> &mut Self {
        self.command_buffer.push_finished_operation(finished_operation);
        self
    }

    /// Special function for depth buffer layout transition
    pub fn depth_pipeline_barrier(&mut self, swapchain_depth_buffer: &SwapchainDepthBuffer) -> crate::Result<&mut Self> {
        let layout_transition_barrier = vk::ImageMemoryBarrier::builder()
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
                &[layout_transition_barrier],
            )
        };
        Ok(self)
    }

    /// Special function for writing into indirect draw commands buffer from compute shader and then reading from it in vertex shader
    pub fn indirect_draw_commands_buffer_pipeline_barrier<T>(&mut self, buffer: &impl Buffer<T>) -> &mut Self {
        let buffer_memory_barrier = vk::BufferMemoryBarrier::builder()
            .buffer(*buffer.as_raw_vulkan())
            .src_access_mask(vk::AccessFlags::SHADER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .offset(0)
            .size(vk::WHOLE_SIZE)
            .build();
        unsafe {
            self.device.as_raw_vulkan().cmd_pipeline_barrier(
                *self.command_buffer.as_raw_vulkan(),
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::VERTEX_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[buffer_memory_barrier],
                &[],
            )
        };
        self
    }

    /// Draw command for indirect draw commands
    pub fn draw_indirect(&mut self, buffer: &impl Buffer<DrawIndirectCommand>, draw_count: usize) -> &mut Self {
        unsafe {
            self.device.as_raw_vulkan().cmd_draw_indirect(
                *self.command_buffer.as_raw_vulkan(),
                *buffer.as_raw_vulkan(),
                0,
                draw_count as u32,
                mem::size_of::<DrawIndirectCommand>() as u32,
            )
        };
        self
    }

    /// Pushes the given `push_constants` to the command buffer
    pub fn push_constants<C>(&mut self, push_constants: &[C]) -> crate::Result<()> {
        let bound_pipeline_layout = self.bound_pipeline_layout.ok_or(Error::NoPipelineBound)?;
        unsafe {
            self.device.as_raw_vulkan().cmd_push_constants(
                *self.command_buffer.as_raw_vulkan(),
                bound_pipeline_layout,
                vk::ShaderStageFlags::ALL,
                0,
                std::slice::from_raw_parts(push_constants.as_ptr() as *const _, push_constants.len() * mem::size_of::<C>()),
            );
        }
        Ok(())
    }

    /// Sets line width of the dynamic pipeline state
    pub fn set_line_width(&mut self, line_width: f32) {
        unsafe {
            self.device
                .as_raw_vulkan()
                .cmd_set_line_width(*self.command_buffer.as_raw_vulkan(), line_width);
        }
    }

    /// Pushes the given descriptors to the command buffer
    pub fn push_descriptors(
        &mut self,
        descriptor_set: u32,
        pipeline_bind_point: PipelineBindPoint,
        push_descriptors: &PushDescriptors,
    ) -> crate::Result<()> {
        let bound_pipeline_layout = self.bound_pipeline_layout.ok_or(Error::NoPipelineBound)?;
        unsafe {
            self.device.extensions.push_descriptor.cmd_push_descriptor_set(
                *self.command_buffer.as_raw_vulkan(),
                pipeline_bind_point.into(),
                bound_pipeline_layout,
                descriptor_set,
                push_descriptors.write_descriptor_sets(),
            );
        }
        Ok(())
    }

    /// Dispatches a compute shader
    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) -> &mut Self {
        unsafe {
            self.device
                .as_raw_vulkan()
                .cmd_dispatch(*self.command_buffer.as_raw_vulkan(), x, y, z);
        }
        self
    }
}
