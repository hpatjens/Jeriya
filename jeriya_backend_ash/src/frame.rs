use std::{iter, sync::Arc};

use base::shader_interface;
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    buffer::BufferUsageFlags,
    command_buffer::CommandBuffer,
    command_buffer_builder::CommandBufferBuilder,
    device::Device,
    frame_index::FrameIndex,
    host_visible_buffer::HostVisibleBuffer,
    immediate_graphics_pipeline::{PushConstants, Topology},
    push_descriptors::PushDescriptors,
    semaphore::Semaphore,
};
use jeriya_shared::{debug_info, nalgebra::Matrix4, winit::window::WindowId};

use crate::{ash_immediate::ImmediateCommand, backend_shared::BackendShared, presenter_shared::PresenterShared, ImmediateRenderingRequest};

pub struct Frame {
    image_available_semaphore: Option<Arc<Semaphore>>,
    rendering_complete_semaphores: Vec<Arc<Semaphore>>,
    rendering_complete_command_buffers: Vec<Arc<CommandBuffer>>,
    per_frame_data_buffer: HostVisibleBuffer<shader_interface::PerFrameData>,
    cameras_buffer: HostVisibleBuffer<shader_interface::Camera>,
    inanimate_mesh_instance_buffer: HostVisibleBuffer<shader_interface::InanimateMeshInstance>,
}

impl Frame {
    pub fn new(window_id: &WindowId, backend_shared: &BackendShared) -> base::Result<Self> {
        let image_available_semaphore = None;
        let rendering_complete_semaphores = Vec::new();
        let rendering_complete_command_buffer = Vec::new();
        let per_frame_data_buffer = HostVisibleBuffer::new(
            &backend_shared.device,
            &[shader_interface::PerFrameData::default(); 1],
            BufferUsageFlags::UNIFORM_BUFFER,
            debug_info!(format!("PerFrameDataBuffer-for-Window{:?}", window_id)),
        )?;
        let cameras_buffer = HostVisibleBuffer::new(
            &backend_shared.device,
            &vec![shader_interface::Camera::default(); backend_shared.renderer_config.maximum_number_of_cameras],
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!(format!("CamerasBuffer-for-Window{:?}", window_id)),
        )?;
        let inanimate_mesh_instance_buffer = HostVisibleBuffer::new(
            &backend_shared.device,
            &vec![
                shader_interface::InanimateMeshInstance::default();
                backend_shared.renderer_config.maximum_number_of_inanimate_mesh_instances
            ],
            BufferUsageFlags::STORAGE_BUFFER,
            debug_info!(format!("InanimateMeshInstanceBuffer-for-Window{:?}", window_id)),
        )?;
        Ok(Self {
            image_available_semaphore,
            rendering_complete_semaphores,
            rendering_complete_command_buffers: rendering_complete_command_buffer,
            per_frame_data_buffer,
            cameras_buffer,
            inanimate_mesh_instance_buffer,
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
        frame_index: &FrameIndex,
        _window_id: &WindowId,
        backend_shared: &BackendShared,
        presenter_shared: &mut PresenterShared,
    ) -> jeriya_shared::Result<()> {
        // Wait for the previous work for the currently occupied frame to finish
        for command_buffer in &self.rendering_complete_command_buffers {
            command_buffer.wait_for_completion()?;
        }
        self.rendering_complete_command_buffers.clear();

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

        // Update Buffers
        self.per_frame_data_buffer.set_memory_unaligned(&[shader_interface::PerFrameData {
            active_camera: presenter_shared.active_camera.index() as u32,
        }])?;
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
        self.inanimate_mesh_instance_buffer.set_memory_unaligned({
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
        })?;

        // Build CommandBuffer
        let mut command_buffer = CommandBuffer::new(
            &backend_shared.device,
            &backend_shared.command_pool,
            debug_info!("CommandBuffer-for-Swapchain-Renderpass"),
        )?;
        let mut command_buffer_builder = CommandBufferBuilder::new(&backend_shared.device, &mut command_buffer)?;
        command_buffer_builder
            .begin_command_buffer_for_one_time_submit()?
            .depth_pipeline_barrier(presenter_shared.depth_buffers().depth_buffers.get(frame_index))?
            .begin_render_pass(
                presenter_shared.swapchain(),
                presenter_shared.render_pass(),
                (presenter_shared.framebuffers(), frame_index.swapchain_index()),
            )?
            .bind_graphics_pipeline(&presenter_shared.simple_graphics_pipeline);
        self.push_descriptors(presenter_shared, &mut command_buffer_builder)?;
        self.append_immediate_rendering_commands(
            &backend_shared.device,
            presenter_shared,
            &mut command_buffer_builder,
            &presenter_shared.immediate_rendering_requests,
        )?;
        command_buffer_builder.end_render_pass()?.end_command_buffer()?;

        // Save CommandBuffer to be able to check whether this frame was completed
        let command_buffer = Arc::new(command_buffer);
        self.rendering_complete_command_buffers.push(command_buffer.clone());

        // Submit immediate rendering
        let image_available_semaphore = self
            .image_available_semaphore
            .as_ref()
            .expect("not image available semaphore assigned for the frame");

        // Insert into Queue
        backend_shared.presentation_queue.borrow_mut().submit_for_rendering_complete(
            command_buffer,
            image_available_semaphore,
            &main_rendering_complete_semaphore,
        )?;

        // Remove all ImmediateRenderingRequests that don't have to be rendered anymore
        presenter_shared.immediate_rendering_requests.clear();

        Ok(())
    }

    /// Pushes the required descriptors to the [`CommandBufferBuilder`].
    fn push_descriptors(&self, presenter_shared: &PresenterShared, command_buffer_builder: &mut CommandBufferBuilder) -> base::Result<()> {
        command_buffer_builder.push_descriptors_for_graphics(0, {
            &PushDescriptors::builder(&presenter_shared.simple_graphics_pipeline.descriptor_set_layout)
                .push_uniform_buffer(0, &self.per_frame_data_buffer)
                .push_storage_buffer(1, &self.cameras_buffer)
                .push_storage_buffer(2, &self.inanimate_mesh_instance_buffer)
                .build()
        })?;
        Ok(())
    }

    fn append_immediate_rendering_commands(
        &self,
        device: &Arc<Device>,
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
            device,
            data.as_slice(),
            BufferUsageFlags::VERTEX_BUFFER,
            debug_info!("Immediate-VertexBuffer"),
        )?);
        command_buffer_builder.bind_vertex_buffers(0, &vertex_buffer);

        // Append the draw commands
        let mut first_vertex = 0;
        let mut last_matrix = Matrix4::identity();
        for immediate_rendering_request in immediate_rendering_requests {
            let mut last_topology = None;
            for command in &immediate_rendering_request.immediate_command_buffer.commands {
                match command {
                    ImmediateCommand::Matrix(matrix) => last_matrix = *matrix,
                    ImmediateCommand::LineList(line_list) => {
                        if !matches!(last_topology, Some(Topology::LineList)) {
                            command_buffer_builder.bind_graphics_pipeline(&presenter_shared.immediate_graphics_pipeline_line_list);
                            self.push_descriptors(presenter_shared, command_buffer_builder)?;
                        }
                        let push_constants = PushConstants {
                            color: line_list.config().color,
                            matrix: last_matrix,
                        };
                        command_buffer_builder.push_constants(&[push_constants])?;
                        command_buffer_builder.set_line_width(line_list.config().line_width);
                        command_buffer_builder.draw_vertices(line_list.positions().len() as u32, first_vertex as u32);
                        first_vertex += line_list.positions().len();
                        last_topology = Some(Topology::LineList);
                    }
                    ImmediateCommand::LineStrip(line_strip) => {
                        if !matches!(last_topology, Some(Topology::LineStrip)) {
                            command_buffer_builder.bind_graphics_pipeline(&presenter_shared.immediate_graphics_pipeline_line_strip);
                            self.push_descriptors(presenter_shared, command_buffer_builder)?;
                        }
                        let push_constants = PushConstants {
                            color: line_strip.config().color,
                            matrix: last_matrix,
                        };
                        command_buffer_builder.push_constants(&[push_constants])?;
                        command_buffer_builder.set_line_width(line_strip.config().line_width);
                        command_buffer_builder.draw_vertices(line_strip.positions().len() as u32, first_vertex as u32);
                        first_vertex += line_strip.positions().len();
                        last_topology = Some(Topology::LineStrip);
                    }
                    ImmediateCommand::TriangleList(triangle_list) => {
                        if !matches!(last_topology, Some(Topology::TriangleList)) {
                            command_buffer_builder.bind_graphics_pipeline(&presenter_shared.immediate_graphics_pipeline_triangle_list);
                            self.push_descriptors(presenter_shared, command_buffer_builder)?;
                        }
                        let push_constants = PushConstants {
                            color: triangle_list.config().color,
                            matrix: last_matrix,
                        };
                        command_buffer_builder.push_constants(&[push_constants])?;
                        command_buffer_builder.draw_vertices(triangle_list.positions().len() as u32, first_vertex as u32);
                        first_vertex += triangle_list.positions().len();
                        last_topology = Some(Topology::TriangleList);
                    }
                    ImmediateCommand::TriangleStrip(triangle_strip) => {
                        if !matches!(last_topology, Some(Topology::TriangleStrip)) {
                            command_buffer_builder.bind_graphics_pipeline(&presenter_shared.immediate_graphics_pipeline_triangle_strip);
                            self.push_descriptors(presenter_shared, command_buffer_builder)?;
                        }
                        let push_constants = PushConstants {
                            color: triangle_strip.config().color,
                            matrix: last_matrix,
                        };
                        command_buffer_builder.push_constants(&[push_constants])?;
                        command_buffer_builder.draw_vertices(triangle_strip.positions().len() as u32, first_vertex as u32);
                        first_vertex += triangle_strip.positions().len();
                        last_topology = Some(Topology::TriangleStrip);
                    }
                }
            }
        }

        Ok(())
    }
}
