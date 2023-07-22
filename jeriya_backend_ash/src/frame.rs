use std::{iter, mem, sync::Arc};

use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    buffer::BufferUsageFlags,
    command_buffer::CommandBuffer,
    command_buffer_builder::CommandBufferBuilder,
    command_buffer_builder::PipelineBindPoint,
    command_pool::{CommandPool, CommandPoolCreateFlags},
    descriptor_set_layout::DescriptorSetLayout,
    device_visible_buffer::DeviceVisibleBuffer,
    graphics_pipeline::PrimitiveTopology,
    host_visible_buffer::HostVisibleBuffer,
    push_descriptors::PushDescriptors,
    queue::Queue,
    semaphore::Semaphore,
    shader_interface, DrawIndirectCommand,
};
use jeriya_macros::profile;
use jeriya_shared::plot_with_index;
use jeriya_shared::{
    debug_info,
    log::info,
    nalgebra::Matrix4,
    tracy_client::{plot, span},
    winit::window::WindowId,
};

use crate::{
    ash_immediate::ImmediateCommand,
    backend_shared::BackendShared,
    presenter_shared::{PresenterShared, PushConstants},
    ImmediateRenderingRequest,
};

pub struct Frame {
    presenter_index: usize,
    image_available_semaphore: Option<Arc<Semaphore>>,
    rendering_complete_semaphores: Vec<Arc<Semaphore>>,
    per_frame_data_buffer: HostVisibleBuffer<shader_interface::PerFrameData>,
    cameras_buffer: HostVisibleBuffer<shader_interface::Camera>,
    inanimate_mesh_instance_buffer: HostVisibleBuffer<shader_interface::InanimateMeshInstance>,
    indirect_draw_buffer: Arc<DeviceVisibleBuffer<DrawIndirectCommand>>,
}

#[profile]
impl Frame {
    pub fn new(presenter_index: usize, window_id: &WindowId, backend_shared: &BackendShared) -> base::Result<Self> {
        let image_available_semaphore = None;
        let rendering_complete_semaphores = Vec::new();
        let per_frame_data_buffer = HostVisibleBuffer::new(
            &backend_shared.device,
            &[shader_interface::PerFrameData::default(); 1],
            BufferUsageFlags::UNIFORM_BUFFER,
            debug_info!(format!("PerFrameDataBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create camera buffer");
        let cameras_buffer = HostVisibleBuffer::new(
            &backend_shared.device,
            &vec![shader_interface::Camera::default(); backend_shared.renderer_config.maximum_number_of_cameras],
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!(format!("CamerasBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create inanimate mesh instance buffer");
        let inanimate_mesh_instance_buffer = HostVisibleBuffer::new(
            &backend_shared.device,
            &vec![
                shader_interface::InanimateMeshInstance::default();
                backend_shared.renderer_config.maximum_number_of_inanimate_mesh_instances
            ],
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!(format!("InanimateMeshInstanceBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create indirect draw buffer");
        let indirect_draw_buffer = DeviceVisibleBuffer::new(
            &backend_shared.device,
            backend_shared.renderer_config.maximum_number_of_inanimate_mesh_instances * mem::size_of::<DrawIndirectCommand>(),
            BufferUsageFlags::INDIRECT_BUFFER | BufferUsageFlags::STORAGE_BUFFER,
            debug_info!(format!("IndirectDrawBuffer-for-Window{:?}", window_id)),
        )?;

        Ok(Self {
            presenter_index,
            image_available_semaphore,
            rendering_complete_semaphores,
            per_frame_data_buffer,
            cameras_buffer,
            inanimate_mesh_instance_buffer,
            indirect_draw_buffer,
        })
    }

    /// Sets the image available semaphore for the frame.
    pub fn set_image_available_semaphore(&mut self, image_available_semaphore: Arc<Semaphore>) {
        self.image_available_semaphore = Some(image_available_semaphore);
    }

    /// Returns the rendering complete semaphores for the frame.
    pub fn rendering_complete_semaphores(&self) -> &[Arc<Semaphore>] {
        &self.rendering_complete_semaphores
    }

    pub fn render_frame(
        &mut self,
        _window_id: &WindowId,
        presentation_queue: &mut Queue,
        backend_shared: &BackendShared,
        presenter_shared: &mut PresenterShared,
        rendering_complete_command_buffer: &mut Option<Arc<CommandBuffer>>,
    ) -> jeriya_shared::Result<()> {
        // Wait for the previous work for the currently occupied frame to finish
        let wait_span = span!("wait for rendering complete");
        if let Some(command_buffer) = rendering_complete_command_buffer.take() {
            command_buffer.wait_for_completion()?;
        }
        drop(wait_span);

        // Prepare rendering complete semaphore
        let main_rendering_complete_semaphore = Arc::new(Semaphore::new(
            &backend_shared.device,
            debug_info!("main-CommandBuffer-rendering-complete-Semaphore"),
        )?);
        self.rendering_complete_semaphores.clear();
        self.rendering_complete_semaphores.push(main_rendering_complete_semaphore.clone());
        assert_eq!(
            self.rendering_complete_semaphores.len(),
            1,
            "There should only be one rendering complete semaphore"
        );

        // Prepare InanimateMeshInstances
        let inanimate_mesh_instances = {
            let _span = span!("prepare inanimate mesh instances");

            let inanimate_mesh_instances = backend_shared.inanimate_mesh_instances.lock();
            let padding = backend_shared.renderer_config.maximum_number_of_inanimate_mesh_instances - inanimate_mesh_instances.len();
            &inanimate_mesh_instances
                .as_slice()
                .iter()
                .map(|inanimate_mesh_instance| shader_interface::InanimateMeshInstance {
                    inanimate_mesh_index: inanimate_mesh_instance.inanimate_mesh.handle().index() as u64,
                    transform: inanimate_mesh_instance.transform.matrix().clone(),
                })
                .chain(iter::repeat(shader_interface::InanimateMeshInstance::default()).take(padding))
                .collect::<Vec<_>>()
        };

        // Update Buffers
        let span = span!("update per frame data buffer");
        self.per_frame_data_buffer.set_memory_unaligned(&[shader_interface::PerFrameData {
            active_camera: presenter_shared.active_camera.index() as u32,
            inanimate_mesh_instance_count: inanimate_mesh_instances.len() as u32,
        }])?;
        drop(span);

        let span = span!("update cameras buffer");
        self.cameras_buffer.set_memory_unaligned({
            let cameras = backend_shared.cameras.lock();
            let padding = backend_shared.renderer_config.maximum_number_of_cameras - cameras.len();
            &cameras
                .as_slice()
                .iter()
                .map(|camera| shader_interface::Camera {
                    projection_matrix: camera.projection_matrix(),
                    view_matrix: camera.view_matrix(),
                    matrix: camera.matrix(),
                })
                .chain(iter::repeat(shader_interface::Camera::default()).take(padding))
                .collect::<Vec<_>>()
        })?;
        drop(span);

        let span = span!("update inanimate_mesh_instances_buffer");
        self.inanimate_mesh_instance_buffer
            .set_memory_unaligned(&inanimate_mesh_instances)?;
        drop(span);

        const LOCAL_SIZE_X: u32 = 128;
        let cull_compute_shader_group_count = (inanimate_mesh_instances.len() as u32 + LOCAL_SIZE_X - 1) / LOCAL_SIZE_X;

        // Create a CommandPool
        let command_pool_span = span!("create commnad pool");
        let command_pool = CommandPool::new(
            &backend_shared.device,
            presentation_queue,
            CommandPoolCreateFlags::ResetCommandBuffer,
            debug_info!("preliminary-CommandPool"),
        )?;
        drop(command_pool_span);

        // Build CommandBuffer
        let command_buffer_span = span!("build command buffer");

        let creation_span = span!("command buffer creation");
        let mut command_buffer = CommandBuffer::new(
            &backend_shared.device,
            &command_pool,
            debug_info!("CommandBuffer-for-Swapchain-Renderpass"),
        )?;
        let mut builder = CommandBufferBuilder::new(&backend_shared.device, &mut command_buffer)?;
        drop(creation_span);

        builder.begin_command_buffer_for_one_time_submit()?;
        builder.depth_pipeline_barrier(presenter_shared.depth_buffers().depth_buffers.get(&presenter_shared.frame_index))?;

        // Cull
        let cull_span = span!("record cull commands");
        builder.bind_compute_pipeline(&presenter_shared.cull_compute_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Compute,
            &presenter_shared.cull_compute_pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.dispatch(cull_compute_shader_group_count, 1, 1);
        builder.indirect_draw_commands_buffer_pipeline_barrier(&self.indirect_draw_buffer);
        drop(cull_span);

        // Render Pass
        builder.begin_render_pass(
            presenter_shared.swapchain(),
            presenter_shared.render_pass(),
            (presenter_shared.framebuffers(), presenter_shared.frame_index.swapchain_index()),
        )?;

        // Render with IndirectGraphicsPipeline
        let indirect_span = span!("record indirect commands");
        builder.bind_graphics_pipeline(&presenter_shared.indirect_graphics_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Graphics,
            &presenter_shared.indirect_graphics_pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.draw_indirect(&self.indirect_draw_buffer, inanimate_mesh_instances.len());
        drop(indirect_span);

        // Render with SimpleGraphicsPipeline
        let simple_span = span!("record simple commands");
        builder.bind_graphics_pipeline(&presenter_shared.simple_graphics_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Graphics,
            &presenter_shared.simple_graphics_pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        drop(simple_span);

        // Render with ImmediateRenderingPipeline
        self.append_immediate_rendering_commands(
            backend_shared,
            presenter_shared,
            &mut builder,
            &presenter_shared.immediate_rendering_requests,
        )?;

        builder.end_render_pass()?;
        builder.end_command_buffer()?;

        drop(command_buffer_span);

        // Save CommandBuffer to be able to check whether this frame was completed
        let command_buffer = Arc::new(command_buffer);
        *rendering_complete_command_buffer = Some(command_buffer.clone());

        // Submit immediate rendering
        let image_available_semaphore = self
            .image_available_semaphore
            .as_ref()
            .expect("not image available semaphore assigned for the frame");

        // Insert into Queue
        let submit_span = span!("submit command buffer commands");
        presentation_queue.submit_for_rendering_complete(command_buffer, image_available_semaphore, &main_rendering_complete_semaphore)?;
        drop(submit_span);

        // Remove all ImmediateRenderingRequests that don't have to be rendered anymore
        presenter_shared.immediate_rendering_requests.clear();

        Ok(())
    }

    /// Pushes the required descriptors to the [`CommandBufferBuilder`].
    fn push_descriptors(
        &self,
        pipeline_bind_point: PipelineBindPoint,
        descriptor_set_layout: &DescriptorSetLayout,
        backend_shared: &BackendShared,
        command_buffer_builder: &mut CommandBufferBuilder,
    ) -> base::Result<()> {
        let push_descriptors = &PushDescriptors::builder(&descriptor_set_layout)
            .push_uniform_buffer(0, &self.per_frame_data_buffer)
            .push_storage_buffer(1, &self.cameras_buffer)
            .push_storage_buffer(2, &self.inanimate_mesh_instance_buffer)
            .push_storage_buffer(3, &self.indirect_draw_buffer)
            .push_storage_buffer(4, &*backend_shared.inanimate_mesh_buffer.lock())
            .push_storage_buffer(5, &*backend_shared.static_vertex_buffer.lock())
            .build();
        command_buffer_builder.push_descriptors(0, pipeline_bind_point, push_descriptors)?;
        Ok(())
    }

    fn append_immediate_rendering_commands(
        &self,
        backend_shared: &BackendShared,
        presenter_shared: &PresenterShared,
        command_buffer_builder: &mut CommandBufferBuilder,
        immediate_rendering_requests: &Vec<ImmediateRenderingRequest>,
    ) -> base::Result<()> {
        if immediate_rendering_requests.is_empty() {
            return Ok(());
        }

        // Collect vertex attributes for all immediate rendering requests
        let mut data = Vec::new();
        for request in immediate_rendering_requests {
            for command in &request.immediate_command_buffer.commands {
                match command {
                    ImmediateCommand::Matrix(..) => {}
                    ImmediateCommand::LineList(line_list) => data.extend_from_slice(line_list.positions()),
                    ImmediateCommand::LineStrip(line_strip) => data.extend_from_slice(line_strip.positions()),
                    ImmediateCommand::TriangleList(triangle_list) => data.extend_from_slice(triangle_list.positions()),
                    ImmediateCommand::TriangleStrip(triangle_strip) => data.extend_from_slice(triangle_strip.positions()),
                }
            }
        }
        let vertex_buffer = Arc::new(HostVisibleBuffer::new(
            &backend_shared.device,
            data.as_slice(),
            BufferUsageFlags::VERTEX_BUFFER,
            debug_info!("Immediate-VertexBuffer"),
        )?);
        command_buffer_builder.bind_vertex_buffers(0, &vertex_buffer);

        plot_with_index!(
            "immediate_rendering_requests_on_presenter_",
            self.presenter_index,
            immediate_rendering_requests.len() as f64
        );

        // Append the draw commands
        let mut first_vertex = 0;
        let mut last_matrix = Matrix4::identity();
        for immediate_rendering_request in immediate_rendering_requests {
            let mut last_topology = None;
            for command in &immediate_rendering_request.immediate_command_buffer.commands {
                match command {
                    ImmediateCommand::Matrix(matrix) => last_matrix = *matrix,
                    ImmediateCommand::LineList(line_list) => {
                        if !matches!(last_topology, Some(PrimitiveTopology::LineList)) {
                            command_buffer_builder.bind_graphics_pipeline(&presenter_shared.immediate_graphics_pipeline_line_list);
                            self.push_descriptors(
                                PipelineBindPoint::Graphics,
                                &presenter_shared.immediate_graphics_pipeline_line_list.descriptor_set_layout,
                                backend_shared,
                                command_buffer_builder,
                            )?;
                        }
                        let push_constants = PushConstants {
                            color: line_list.config().color,
                            matrix: last_matrix,
                        };
                        command_buffer_builder.push_constants(&[push_constants])?;
                        command_buffer_builder.set_line_width(line_list.config().line_width);
                        command_buffer_builder.draw_vertices(line_list.positions().len() as u32, first_vertex as u32);
                        first_vertex += line_list.positions().len();
                        last_topology = Some(PrimitiveTopology::LineList);
                    }
                    ImmediateCommand::LineStrip(line_strip) => {
                        if !matches!(last_topology, Some(PrimitiveTopology::LineStrip)) {
                            command_buffer_builder.bind_graphics_pipeline(&presenter_shared.immediate_graphics_pipeline_line_strip);
                            self.push_descriptors(
                                PipelineBindPoint::Graphics,
                                &presenter_shared.immediate_graphics_pipeline_line_strip.descriptor_set_layout,
                                backend_shared,
                                command_buffer_builder,
                            )?;
                        }
                        let push_constants = PushConstants {
                            color: line_strip.config().color,
                            matrix: last_matrix,
                        };
                        command_buffer_builder.push_constants(&[push_constants])?;
                        command_buffer_builder.set_line_width(line_strip.config().line_width);
                        command_buffer_builder.draw_vertices(line_strip.positions().len() as u32, first_vertex as u32);
                        first_vertex += line_strip.positions().len();
                        last_topology = Some(PrimitiveTopology::LineStrip);
                    }
                    ImmediateCommand::TriangleList(triangle_list) => {
                        if !matches!(last_topology, Some(PrimitiveTopology::TriangleList)) {
                            command_buffer_builder.bind_graphics_pipeline(&presenter_shared.immediate_graphics_pipeline_triangle_list);
                            self.push_descriptors(
                                PipelineBindPoint::Graphics,
                                &presenter_shared.immediate_graphics_pipeline_triangle_list.descriptor_set_layout,
                                backend_shared,
                                command_buffer_builder,
                            )?;
                        }
                        let push_constants = PushConstants {
                            color: triangle_list.config().color,
                            matrix: last_matrix,
                        };
                        command_buffer_builder.push_constants(&[push_constants])?;
                        command_buffer_builder.draw_vertices(triangle_list.positions().len() as u32, first_vertex as u32);
                        first_vertex += triangle_list.positions().len();
                        last_topology = Some(PrimitiveTopology::TriangleList);
                    }
                    ImmediateCommand::TriangleStrip(triangle_strip) => {
                        if !matches!(last_topology, Some(PrimitiveTopology::TriangleStrip)) {
                            command_buffer_builder.bind_graphics_pipeline(&presenter_shared.immediate_graphics_pipeline_triangle_strip);
                            self.push_descriptors(
                                PipelineBindPoint::Graphics,
                                &presenter_shared.immediate_graphics_pipeline_triangle_strip.descriptor_set_layout,
                                backend_shared,
                                command_buffer_builder,
                            )?;
                        }
                        let push_constants = PushConstants {
                            color: triangle_strip.config().color,
                            matrix: last_matrix,
                        };
                        command_buffer_builder.push_constants(&[push_constants])?;
                        command_buffer_builder.draw_vertices(triangle_strip.positions().len() as u32, first_vertex as u32);
                        first_vertex += triangle_strip.positions().len();
                        last_topology = Some(PrimitiveTopology::TriangleStrip);
                    }
                }
            }
        }

        Ok(())
    }
}
