use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use crate::{
    ash_immediate::ImmediateCommand, ash_shared_backend::AshSharedBackend, presenter_resources::PresenterResources,
    ImmediateRenderingRequest,
};
use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    buffer::BufferUsageFlags, command_buffer::CommandBuffer, command_buffer_builder::CommandBufferBuilder, device::Device,
    frame_index::FrameIndex, host_visible_buffer::HostVisibleBuffer, immediate_graphics_pipeline::ImmediateGraphicsPipeline,
    immediate_graphics_pipeline::PushConstants, immediate_graphics_pipeline::Topology, push_descriptors::PushDescriptors,
    semaphore::Semaphore, shader_interface::PerFrameData, simple_graphics_pipeline::SimpleGraphicsPipeline, surface::Surface,
    swapchain_vec::SwapchainVec,
};
use jeriya_shared::{debug_info, nalgebra::Matrix4, parking_lot::Mutex, winit::window::WindowId, Camera, CameraContainerGuard, Handle};

pub struct Presenter {
    frame_index: FrameIndex,
    frame_index_history: VecDeque<FrameIndex>,
    active_camera: Handle<Camera>,
    presenter_resources: PresenterResources,
    image_available_semaphore: SwapchainVec<Option<Arc<Semaphore>>>,
    rendering_complete_semaphores: SwapchainVec<Vec<Arc<Semaphore>>>,
    rendering_complete_command_buffers: SwapchainVec<Vec<Arc<CommandBuffer>>>,
    simple_graphics_pipeline: SimpleGraphicsPipeline,
    immediate_graphics_pipeline_line_list: ImmediateGraphicsPipeline,
    immediate_graphics_pipeline_line_strip: ImmediateGraphicsPipeline,
    immediate_graphics_pipeline_triangle_list: ImmediateGraphicsPipeline,
    immediate_graphics_pipeline_triangle_strip: ImmediateGraphicsPipeline,
    cameras_buffer: SwapchainVec<HostVisibleBuffer<PerFrameData>>,
}

impl Presenter {
    pub fn new(window_id: &WindowId, surface: &Arc<Surface>, shared_backend: &AshSharedBackend) -> jeriya_shared::Result<Self> {
        let presenter_resources = PresenterResources::new(
            &shared_backend.device,
            surface,
            shared_backend.renderer_config.default_desired_swapchain_length,
        )?;
        let image_available_semaphore = SwapchainVec::new(presenter_resources.swapchain(), |_| Ok(None))?;
        let rendering_complete_semaphores = SwapchainVec::new(presenter_resources.swapchain(), |_| Ok(Vec::new()))?;
        let rendering_complete_command_buffer = SwapchainVec::new(presenter_resources.swapchain(), |_| Ok(Vec::new()))?;
        let frame_index = FrameIndex::new();

        // Graphics Pipeline
        let simple_graphics_pipeline = SimpleGraphicsPipeline::new(
            &shared_backend.device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("SimpleGraphicsPipeline-for-Window{:?}", window_id)),
        )?;
        let immediate_graphics_pipeline_line_list = ImmediateGraphicsPipeline::new(
            &shared_backend.device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
            Topology::LineList,
        )?;
        let immediate_graphics_pipeline_line_strip = ImmediateGraphicsPipeline::new(
            &shared_backend.device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
            Topology::LineStrip,
        )?;
        let immediate_graphics_pipeline_triangle_list = ImmediateGraphicsPipeline::new(
            &shared_backend.device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
            Topology::TriangleList,
        )?;
        let immediate_graphics_pipeline_triangle_strip = ImmediateGraphicsPipeline::new(
            &shared_backend.device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
            Topology::TriangleStrip,
        )?;

        // Create a camera for this window
        let mut guard = CameraContainerGuard::new(
            shared_backend.camera_event_queue.lock(),
            shared_backend.cameras.lock(),
            shared_backend.renderer_config.clone(),
        );
        let active_camera = guard.insert(Camera::default())?;
        drop(guard);

        let cameras_buffer = SwapchainVec::new(presenter_resources.swapchain(), |_| {
            Ok(HostVisibleBuffer::new(
                &shared_backend.device,
                &vec![PerFrameData::default(); 1],
                BufferUsageFlags::UNIFORM_BUFFER,
                debug_info!(format!("CamerasBuffer-for-Window{:?}", window_id)),
            )?)
        })?;

        Ok(Self {
            frame_index,
            presenter_resources,
            image_available_semaphore,
            rendering_complete_semaphores,
            rendering_complete_command_buffers: rendering_complete_command_buffer,
            frame_index_history: VecDeque::new(),
            simple_graphics_pipeline,
            immediate_graphics_pipeline_line_list,
            immediate_graphics_pipeline_line_strip,
            immediate_graphics_pipeline_triangle_list,
            immediate_graphics_pipeline_triangle_strip,
            cameras_buffer,
            active_camera,
        })
    }

    pub fn render_frame(&mut self, window_id: &WindowId, shared_backend: &AshSharedBackend) -> jeriya_shared::Result<()> {
        // Acquire the next swapchain index
        let image_available_semaphore = Arc::new(Semaphore::new(&shared_backend.device, debug_info!("image-available-Semaphore"))?);
        let frame_index = self
            .presenter_resources
            .swapchain()
            .acquire_next_image(&mut self.frame_index(), &image_available_semaphore)?;
        self.start_frame(frame_index.clone());
        self.image_available_semaphore
            .replace(&self.frame_index(), image_available_semaphore);

        // Wait for the previous work for the currently occupied frame to finish
        for command_buffer in self.rendering_complete_command_buffers.get(&self.frame_index()) {
            command_buffer.wait_for_completion()?;
        }
        self.rendering_complete_command_buffers.get_mut(&self.frame_index()).clear();

        // Prepare rendering complete semaphore
        let main_rendering_complete_semaphore = Arc::new(Semaphore::new(
            &shared_backend.device,
            debug_info!("main-CommandBuffer-rendering-complete-Semaphore"),
        )?);
        let rendering_complete_semaphores = self.rendering_complete_semaphores.get_mut(&self.frame_index());
        rendering_complete_semaphores.clear();
        rendering_complete_semaphores.push(main_rendering_complete_semaphore.clone());
        assert_eq!(
            rendering_complete_semaphores.len(),
            1,
            "There should only be one rendering complete semaphore"
        );

        // Build CommandBuffer
        let mut command_buffer = CommandBuffer::new(
            &shared_backend.device,
            &shared_backend.command_pool,
            debug_info!("CommandBuffer-for-Swapchain-Renderpass"),
        )?;
        let mut command_buffer_builder = CommandBufferBuilder::new(&shared_backend.device, &mut command_buffer)?;
        command_buffer_builder
            .begin_command_buffer_for_one_time_submit()?
            .depth_pipeline_barrier(self.presenter_resources.depth_buffers().depth_buffers.get(&self.frame_index()))?
            .begin_render_pass(
                self.presenter_resources.swapchain(),
                self.presenter_resources.render_pass(),
                (self.presenter_resources.framebuffers(), self.frame_index().swapchain_index()),
            )?
            .bind_graphics_pipeline(&self.simple_graphics_pipeline);
        self.push_descriptors(&self, &mut command_buffer_builder)?;
        self.append_immediate_rendering_commands(
            &shared_backend.device,
            window_id,
            self,
            &mut command_buffer_builder,
            &shared_backend.immediate_rendering_requests,
        )?;
        command_buffer_builder.end_render_pass()?.end_command_buffer()?;

        // Save CommandBuffer to be able to check whether this frame was completed
        let command_buffer = Arc::new(command_buffer);
        self.rendering_complete_command_buffers
            .get_mut(&self.frame_index())
            .push(command_buffer.clone());

        // Submit immediate rendering
        let image_available_semaphore = self
            .image_available_semaphore
            .get(&self.frame_index())
            .as_ref()
            .expect("not image available semaphore assigned for the frame");

        // Insert into Queue
        shared_backend.presentation_queue.borrow_mut().submit_for_rendering_complete(
            command_buffer,
            &image_available_semaphore,
            &main_rendering_complete_semaphore,
        )?;

        // Present
        self.presenter_resources.swapchain().present(
            &self.frame_index(),
            &self.rendering_complete_semaphores.get(&self.frame_index()),
            &shared_backend.presentation_queue.borrow(),
        )?;

        Ok(())
    }

    /// Recreates the [`PresenterResources`] in case of a swapchain resize
    pub fn recreate(&mut self) -> core::Result<()> {
        self.presenter_resources.recreate()
    }

    /// Sets the given [`FrameIndex`] and appends the previous one to the history
    pub fn start_frame(&mut self, frame_index: FrameIndex) {
        self.frame_index_history.push_front(self.frame_index.clone());
        self.frame_index = frame_index;
        while self.frame_index_history.len() > self.presenter_resources.swapchain().len() - 1 {
            self.frame_index_history.pop_back();
        }
    }

    /// Returns the current [`FrameIndex`]
    pub fn frame_index(&self) -> FrameIndex {
        self.frame_index.clone()
    }

    /// Sets the active camera
    pub fn set_active_camera(&mut self, handle: Handle<Camera>) {
        self.active_camera = handle;
    }

    /// Returns the [`FrameIndex`] of the oldest frame in the history
    #[allow(dead_code)]
    pub fn oldest_frame_index(&self) -> Option<FrameIndex> {
        self.frame_index_history.back().cloned()
    }

    /// Pushes the required descriptors to the [`CommandBufferBuilder`].
    fn push_descriptors(&self, presenter: &Presenter, command_buffer_builder: &mut CommandBufferBuilder) -> core::Result<()> {
        command_buffer_builder.push_descriptors_for_graphics(0, {
            let cameras_buffer = presenter.cameras_buffer.get(&presenter.frame_index());
            &PushDescriptors::builder(&presenter.simple_graphics_pipeline.descriptor_set_layout)
                .push_uniform_buffer(0, cameras_buffer)
                .build()
        })?;
        Ok(())
    }

    fn append_immediate_rendering_commands(
        &self,
        device: &Arc<Device>,
        window_id: &WindowId,
        presenter: &Presenter,
        command_buffer_builder: &mut CommandBufferBuilder,
        immediate_rendering_requests: &Mutex<HashMap<WindowId, Vec<ImmediateRenderingRequest>>>,
    ) -> core::Result<()> {
        let mut immediate_rendering_requests = immediate_rendering_requests.lock();
        if let Some(requests) = immediate_rendering_requests.get_mut(window_id) {
            // Collect vertex attributes for all immediate rendering requests
            assert!(!requests.is_empty(), "Vecs should be removed when they are empty");
            let mut data = Vec::new();
            for request in &mut *requests {
                assert!(request.count > 0, "Count must be greater than 0");
                request.count -= 1;
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
                &device,
                data.as_slice(),
                BufferUsageFlags::VERTEX_BUFFER,
                debug_info!("Immediate-VertexBuffer"),
            )?);
            command_buffer_builder.bind_vertex_buffers(0, &vertex_buffer);

            // Append the draw commands
            let mut first_vertex = 0;
            let mut last_matrix = Matrix4::identity();
            for immediate_command_buffer in &*requests {
                let mut last_topology = None;
                for command in &immediate_command_buffer.immediate_command_buffer.commands {
                    match command {
                        ImmediateCommand::Matrix(matrix) => last_matrix = *matrix,
                        ImmediateCommand::LineList(line_list) => {
                            if !matches!(last_topology, Some(Topology::LineList)) {
                                command_buffer_builder.bind_graphics_pipeline(&presenter.immediate_graphics_pipeline_line_list);
                                self.push_descriptors(&presenter, command_buffer_builder)?;
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
                                command_buffer_builder.bind_graphics_pipeline(&presenter.immediate_graphics_pipeline_line_strip);
                                self.push_descriptors(&presenter, command_buffer_builder)?;
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
                                command_buffer_builder.bind_graphics_pipeline(&presenter.immediate_graphics_pipeline_triangle_list);
                                self.push_descriptors(&presenter, command_buffer_builder)?;
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
                                command_buffer_builder.bind_graphics_pipeline(&presenter.immediate_graphics_pipeline_triangle_strip);
                                self.push_descriptors(&presenter, command_buffer_builder)?;
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
        }
        Ok(())
    }
}
