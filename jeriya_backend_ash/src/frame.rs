use std::collections::{BTreeMap, VecDeque};
use std::{mem, sync::Arc};

use base::frame_local_buffer::FrameLocalBuffer;
use jeriya_backend::elements::{camera, point_cloud};
use jeriya_backend::instances::{camera_instance, point_cloud_instance};
use jeriya_backend::{
    elements::rigid_mesh,
    instances::rigid_mesh_instance,
    transactions::{self, Transaction},
};
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
    semaphore::Semaphore,
    shader_interface, DispatchIndirectCommand, DrawIndirectCommand,
};
use jeriya_macros::profile;
use jeriya_shared::{
    debug_info,
    log::info,
    nalgebra::Matrix4,
    plot_with_index,
    tracy_client::{plot, span},
    winit::window::WindowId,
};

use crate::ash_immediate::ImmediateRenderingFrameTask;
use crate::{
    ash_immediate::ImmediateCommand,
    backend_shared::BackendShared,
    presenter_shared::{PresenterShared, PushConstants},
};

pub struct Frame {
    presenter_index: usize,
    image_available_semaphore: Option<Arc<Semaphore>>,
    rendering_complete_semaphore: Option<Arc<Semaphore>>,
    per_frame_data_buffer: HostVisibleBuffer<shader_interface::PerFrameData>,

    mesh_attributes_active_buffer: FrameLocalBuffer<u32>, // every u32 represents a bool
    point_cloud_attributes_active_buffer: FrameLocalBuffer<u32>, // every u32 represents a bool
    camera_buffer: FrameLocalBuffer<shader_interface::Camera>,
    camera_instance_buffer: FrameLocalBuffer<shader_interface::CameraInstance>,
    rigid_mesh_buffer: FrameLocalBuffer<shader_interface::RigidMesh>,
    rigid_mesh_instance_buffer: FrameLocalBuffer<shader_interface::RigidMeshInstance>,
    point_cloud_buffer: FrameLocalBuffer<shader_interface::PointCloud>,
    point_cloud_instance_buffer: FrameLocalBuffer<shader_interface::PointCloudInstance>,

    /// Contains the VkIndirectDrawCommands for the visible rigid mesh instances that will
    /// be rendered with the simple mesh representation and not with meshlets.
    visible_rigid_mesh_instances_simple_buffer: Arc<DeviceVisibleBuffer<u32>>,
    /// Contains the indices of the visible rigid mesh instances.
    /// At the front of the buffer is a counter that contains the number of visible instances.
    visible_rigid_mesh_instances: Arc<DeviceVisibleBuffer<u32>>,
    /// Contains the indices of the visible meshlets of the visible rigid mesh instances.
    /// At the front of the buffer is a counter that contains the number of visible meshlets.
    visible_rigid_mesh_meshlets: Arc<DeviceVisibleBuffer<u32>>,
    /// Contains the indices of the visible point cloud instances as well as the indirect
    /// rendering commands preceded with a counter that contains the number of visible instances.
    visible_point_cloud_instances_buffer: Arc<DeviceVisibleBuffer<u32>>,
    transactions: VecDeque<Transaction>,
}

#[profile]
impl Frame {
    pub fn new(presenter_index: usize, window_id: &WindowId, backend_shared: &BackendShared) -> base::Result<Self> {
        let per_frame_data_buffer = HostVisibleBuffer::new(
            &backend_shared.device,
            &[shader_interface::PerFrameData::default(); 1],
            BufferUsageFlags::UNIFORM_BUFFER,
            debug_info!(format!("PerFrameDataBuffer-for-Window{:?}", window_id)),
        )?;

        // Create camera buffer
        let len = backend_shared.renderer_config.maximum_number_of_cameras;
        info!("Create camera buffer with length: {len}");
        let camera_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("CameraBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_camera_instances;
        info!("Create camera instance buffer with length: {len}");
        let camera_instance_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("CameraInstanceBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_mesh_attributes;
        info!("Create mesh attributes active buffer with length: {len}");
        let mesh_attributes_active_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("MeshAttributesActiveBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_point_cloud_attributes;
        info!("Create point cloud attributes active buffer with length: {len}");
        let point_cloud_attributes_active_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("PointCloudAttributesActiveBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_rigid_meshes;
        info!("Create rigid mesh buffer with length: {len}");
        let rigid_mesh_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("RigidMeshBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_rigid_mesh_instances;
        info!("Create rigid mesh instance buffer with length: {len}");
        let rigid_mesh_instance_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("RigidMeshInstanceBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_point_clouds;
        info!("Create point cloud buffer with length: {len}");
        let point_cloud_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("PointCloudBuffer-for-Window{:?}", window_id)),
        )?;

        let len = backend_shared.renderer_config.maximum_number_of_point_cloud_instances;
        info!("Create point cloud instance buffer with length: {len}");
        let point_cloud_instance_buffer = FrameLocalBuffer::new(
            &backend_shared.device,
            len,
            debug_info!(format!("PointCloudInstanceBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create indirect draw buffer");
        let byte_size_draw_indirect_commands =
            backend_shared.renderer_config.maximum_number_of_rigid_mesh_instances * mem::size_of::<DrawIndirectCommand>();
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_rigid_mesh_instance_indices =
            backend_shared.renderer_config.maximum_number_of_rigid_mesh_instances * mem::size_of::<u32>();
        let visible_rigid_mesh_instances_simple_buffer = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_count + byte_size_draw_indirect_commands + byte_size_rigid_mesh_instance_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("IndirectDrawBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create visible rigid mesh instances buffer");
        let byte_size_dispatch_indirect_command = mem::size_of::<DispatchIndirectCommand>();
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_indices = backend_shared.renderer_config.maximum_visible_rigid_mesh_instances * mem::size_of::<u32>();
        let visible_rigid_mesh_instances = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_dispatch_indirect_command + byte_size_count + byte_size_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("VisibleRigidMeshInstancesBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create visible rigid mesh meshlets buffer");
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_draw_indirect_commands =
            backend_shared.renderer_config.maximum_visible_rigid_mesh_meshlets * mem::size_of::<DrawIndirectCommand>();
        let byte_size_meshlet_indices = backend_shared.renderer_config.maximum_visible_rigid_mesh_meshlets * mem::size_of::<u32>();
        let byte_size_rigid_mesh_instance_indices =
            backend_shared.renderer_config.maximum_visible_rigid_mesh_meshlets * mem::size_of::<u32>();
        let visible_rigid_mesh_meshlets = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_count + byte_size_meshlet_indices + byte_size_draw_indirect_commands + byte_size_rigid_mesh_instance_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("VisibleRigidMeshMeshletsBuffer-for-Window{:?}", window_id)),
        )?;

        info!("Create visible point cloud instances buffer");
        let byte_size_count = mem::size_of::<u32>();
        let byte_size_draw_indirect_commands =
            backend_shared.renderer_config.maximum_number_of_point_cloud_instances * mem::size_of::<DrawIndirectCommand>();
        let byte_size_point_cloud_instance_indices =
            backend_shared.renderer_config.maximum_number_of_point_cloud_instances * mem::size_of::<u32>();
        let visible_point_cloud_instances_buffer = DeviceVisibleBuffer::new(
            &backend_shared.device,
            byte_size_count + byte_size_draw_indirect_commands + byte_size_point_cloud_instance_indices,
            // BufferUsageFlags::TRANSFER_SRC_BIT is only needed for debugging
            BufferUsageFlags::STORAGE_BUFFER
                | BufferUsageFlags::INDIRECT_BUFFER
                | BufferUsageFlags::TRANSFER_DST_BIT
                | BufferUsageFlags::TRANSFER_SRC_BIT,
            debug_info!(format!("VisiblePointCloudInstancesBuffer-for-Window{:?}", window_id)),
        )?;

        Ok(Self {
            presenter_index,
            image_available_semaphore: None,
            rendering_complete_semaphore: None,
            per_frame_data_buffer,
            mesh_attributes_active_buffer,
            point_cloud_attributes_active_buffer,
            camera_buffer,
            camera_instance_buffer,
            rigid_mesh_buffer,
            rigid_mesh_instance_buffer,
            point_cloud_buffer,
            point_cloud_instance_buffer,
            visible_rigid_mesh_instances_simple_buffer,
            visible_rigid_mesh_instances,
            visible_rigid_mesh_meshlets,
            visible_point_cloud_instances_buffer,
            transactions: VecDeque::new(),
        })
    }

    /// Pushes a [`Transaction`] to the frame to be processed when the frame is rendered.
    pub fn push_transaction(&mut self, transaction: Transaction) {
        self.transactions.push_back(transaction);
    }

    /// Sets the image available semaphore for the frame.
    pub fn set_image_available_semaphore(&mut self, image_available_semaphore: Arc<Semaphore>) {
        self.image_available_semaphore = Some(image_available_semaphore);
    }

    /// Returns the rendering complete semaphores for the frame.
    pub fn rendering_complete_semaphore(&self) -> Option<&Arc<Semaphore>> {
        self.rendering_complete_semaphore.as_ref()
    }

    pub fn render_frame(
        &mut self,
        window_id: &WindowId,
        backend_shared: &BackendShared,
        presenter_shared: &mut PresenterShared,
        immediate_rendering_frames: &BTreeMap<&'static str, ImmediateRenderingFrameTask>,
        rendering_complete_command_buffer: &mut Option<Arc<CommandBuffer>>,
    ) -> jeriya_backend::Result<()> {
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
        self.rendering_complete_semaphore = Some(main_rendering_complete_semaphore.clone());

        // Process Transactions
        self.process_transactions()?;

        // Update Buffers
        let span = span!("update per frame data buffer");
        let per_frame_data = shader_interface::PerFrameData {
            active_camera: presenter_shared.active_camera_instance.map(|c| c.index() as i32).unwrap_or(-1),
            mesh_attributes_count: self.mesh_attributes_active_buffer.high_water_mark() as u32,
            rigid_mesh_count: self.rigid_mesh_buffer.high_water_mark() as u32,
            rigid_mesh_instance_count: self.rigid_mesh_instance_buffer.high_water_mark() as u32,
            point_cloud_instance_count: self.point_cloud_instance_buffer.high_water_mark() as u32,
        };
        self.per_frame_data_buffer.set_memory_unaligned(&[per_frame_data])?;
        drop(span);

        const LOCAL_SIZE_X: u32 = 128;
        let cull_compute_shader_group_count = (self.rigid_mesh_instance_buffer.high_water_mark() as u32 + LOCAL_SIZE_X - 1) / LOCAL_SIZE_X;

        // Create a CommandPool
        let command_pool_span = span!("create commnad pool");
        let mut queues = backend_shared.queue_scheduler.queues();
        let command_pool = CommandPool::new(
            &backend_shared.device,
            queues.presentation_queue(*window_id),
            CommandPoolCreateFlags::ResetCommandBuffer,
            debug_info!("preliminary-CommandPool"),
        )?;
        drop(queues);
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

        let begin_span = span!("begin command buffer");
        builder.begin_command_buffer_for_one_time_submit()?;
        drop(begin_span);

        // Wait for everything to be finished
        builder.bottom_to_top_pipeline_barrier();

        // Image transition to optimal layout
        builder.depth_pipeline_barrier(presenter_shared.depth_buffers().depth_buffers.get(&presenter_shared.frame_index))?;

        // Rigid Mesh Culling
        //
        // The culling of the rigid mesh instances is done in two steps:
        //
        // 1. Cull the rigid mesh instances: Indices of the visible `RigidMeshInstance`s are written to
        //    the `visible_rigid_mesh_instances` buffer. The number of visible `RigidMeshInstance`s is
        //    written to the front of the buffer by an atomic operation turning the buffer into a bump
        //    allocator. For every `RigidMeshInstance` a compute shader invocation is dispatched.
        //
        // 2. Cull the meshlets of the visible rigid mesh instances: Indices of the visible meshlets are
        //    written to the `visible_rigid_mesh_meshlets` buffer. The number of visible meshlets is
        //    written to the front of the buffer as in step 1. A 2-dimensional compute shader dispatch
        //    is used where the first dimension maps to the visible rigid mesh instances and the
        //    second dimension is a constant value that approximates the lowest expected number of
        //    meshlets per `RigidMeshInstance`. Inside the shader, a loop is used to iterate over all
        //    meshlets of the `RigidMeshInstance` writing the indices of the visible meshlets to the
        //    buffer.

        // 1. Cull RigidMeshInstances
        let cull_rigid_mesh_instances_span = span!("cull rigid mesh instances");
        builder.bind_compute_pipeline(&presenter_shared.graphics_pipelines.cull_rigid_mesh_instances_compute_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Compute,
            &presenter_shared
                .graphics_pipelines
                .cull_rigid_mesh_instances_compute_pipeline
                .descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;

        // Make sure that all indirect read operations are finished before writing to the buffer
        builder.indirect_to_compute_command_pipeline_barrier();
        builder.indirect_to_transfer_command_pipeline_barrier();
        builder.compute_to_compute_pipeline_barrier();

        // Clear counter and VkDispatchIndirectCommand for the visible rigid mesh instances
        // that will be rendered with the meshlet representation.
        let clear_bytes_count = mem::size_of::<DispatchIndirectCommand>() + mem::size_of::<u32>();
        builder.transfer_to_transfer_command_barrier();
        builder.fill_buffer(&self.visible_rigid_mesh_instances, 0, clear_bytes_count as u64, 0);

        // Clear counter for the visible rigid mesh instances that will be rendered with the
        // simple mesh representation.
        let clear_bytes_count = mem::size_of::<u32>();
        builder.transfer_to_transfer_command_barrier();
        builder.fill_buffer(&self.visible_rigid_mesh_instances_simple_buffer, 0, clear_bytes_count as u64, 0);

        // Dispatch compute shader for every rigid mesh instance
        builder.transfer_to_compute_pipeline_barrier();
        builder.dispatch(cull_compute_shader_group_count, 1, 1);
        drop(cull_rigid_mesh_instances_span);

        // {
        //     let mut queues = backend_shared.queue_scheduler.queues();
        //     let buffer = self
        //         .visible_rigid_mesh_instances
        //         .read_into_new_buffer_and_wait(queues.presentation_queue(*window_id), &command_pool)
        //         .unwrap();
        //     let work_group_x = buffer.get_memory_unaligned_index(0).unwrap();
        //     let work_group_y = buffer.get_memory_unaligned_index(1).unwrap();
        //     let work_group_z = buffer.get_memory_unaligned_index(2).unwrap();
        //     let count = buffer.get_memory_unaligned_index(4).unwrap();
        //     eprintln!("instances: {count}, work_group: ({work_group_x}, {work_group_y}, {work_group_z})",);
        // }

        // Cull Meshlets
        let cull_meshlets_span = span!("cull meshlets");
        builder.bind_compute_pipeline(&presenter_shared.graphics_pipelines.cull_rigid_mesh_meshlets_compute_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Compute,
            &presenter_shared
                .graphics_pipelines
                .cull_rigid_mesh_meshlets_compute_pipeline
                .descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;

        // Clear counter for the visible meshlets
        builder.fill_buffer(&self.visible_rigid_mesh_meshlets, 0, mem::size_of::<u32>() as u64, 0);

        builder.transfer_to_indirect_command_barrier();
        builder.transfer_to_compute_pipeline_barrier();
        builder.compute_to_indirect_command_pipeline_barrier();
        builder.compute_to_compute_pipeline_barrier();

        // Dispatch compute shader for every visible rigid mesh instance
        builder.dispatch_indirect(&self.visible_rigid_mesh_instances, 0);
        builder.compute_to_indirect_command_pipeline_barrier();
        drop(cull_meshlets_span);

        // {
        //     let mut queues = backend_shared.queue_scheduler.queues();
        //     let buffer = self
        //         .visible_rigid_mesh_meshlets
        //         .read_into_new_buffer_and_wait(queues.presentation_queue(*window_id), &command_pool)
        //         .unwrap();
        //     let count = buffer.get_memory_unaligned_index(0).unwrap();
        //     let offset = 1 + backend_shared.renderer_config.maximum_visible_rigid_mesh_meshlets * 4;
        //     let list = (0..10)
        //         .map(|i| buffer.get_memory_unaligned_index(offset + i).unwrap())
        //         .collect::<Vec<_>>();
        //     eprintln!("meshlets: {count} -> {list:?}");
        // }

        // Point Cloud Culling
        //
        // The culling of the point cloud instances is done in a single step. The instances are
        // culled by a compute shader that writes the indices of the visible point cloud instances
        // to the `visible_point_cloud_instances` buffer. The number of visible point cloud instances
        // is written to the front of the buffer as in the culling of the rigid mesh instances.
        let cull_point_cloud_instances_span = span!("cull point cloud instances");
        builder.bind_compute_pipeline(&presenter_shared.graphics_pipelines.cull_point_cloud_instances_compute_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Compute,
            &presenter_shared
                .graphics_pipelines
                .cull_point_cloud_instances_compute_pipeline
                .descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;

        // Clear counter for the visible point cloud instances
        builder.fill_buffer(&self.visible_point_cloud_instances_buffer, 0, mem::size_of::<u32>() as u64, 0);

        // Dispatch
        let cull_point_cloud_instances_group_count = {
            const LOCAL_SIZE_X: u32 = 128;
            (self.point_cloud_instance_buffer.high_water_mark() as u32 + LOCAL_SIZE_X - 1) / LOCAL_SIZE_X
        };
        builder.transfer_to_indirect_command_barrier();
        builder.transfer_to_compute_pipeline_barrier();
        builder.dispatch(cull_point_cloud_instances_group_count, 1, 1);

        builder.compute_to_indirect_command_pipeline_barrier();
        drop(cull_point_cloud_instances_span);

        // {
        //     let mut queues = backend_shared.queue_scheduler.queues();
        //     let buffer = self
        //         .visible_point_cloud_instances_buffer
        //         .read_into_new_buffer_and_wait(queues.presentation_queue(*window_id), &command_pool)
        //         .unwrap();
        //     let count = buffer.get_memory_unaligned_index(0).unwrap();
        //     let offset = 1;
        //     let list = (0..10)
        //         .map(|i| buffer.get_memory_unaligned_index(offset + i).unwrap())
        //         .collect::<Vec<_>>();
        //     eprintln!("point_clouds: {count} -> {list:?}");
        // }

        // Render Pass
        builder.begin_render_pass(
            presenter_shared.swapchain(),
            presenter_shared.render_pass(),
            (presenter_shared.framebuffers(), presenter_shared.frame_index.swapchain_index()),
        )?;

        // Render with IndirectSimpleGraphicsPipeline
        let indirect_simple_span = span!("record indirect simple commands");
        builder.bind_graphics_pipeline(&presenter_shared.graphics_pipelines.indirect_simple_graphics_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Graphics,
            &presenter_shared
                .graphics_pipelines
                .indirect_simple_graphics_pipeline
                .descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.draw_indirect_count(
            &self.visible_rigid_mesh_instances_simple_buffer,
            mem::size_of::<u32>() as u64,
            &self.visible_rigid_mesh_instances_simple_buffer,
            0,
            self.rigid_mesh_instance_buffer.high_water_mark(),
        );
        drop(indirect_simple_span);

        // Render with IndirectMeshletGraphicsPipeline
        let indirect_meshlet_span = span!("record indirect meshlet commands");
        builder.bind_graphics_pipeline(&presenter_shared.graphics_pipelines.indirect_meshlet_graphics_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Graphics,
            &presenter_shared
                .graphics_pipelines
                .indirect_meshlet_graphics_pipeline
                .descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.draw_indirect_count(
            &self.visible_rigid_mesh_meshlets,
            mem::size_of::<u32>() as u64,
            &self.visible_rigid_mesh_meshlets,
            0,
            backend_shared.static_meshlet_buffer.lock().len(),
        );
        drop(indirect_meshlet_span);

        // Render Point Clouds
        let point_cloud_span = span!("record point cloud commands");
        builder.bind_graphics_pipeline(&presenter_shared.graphics_pipelines.point_cloud_graphics_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Graphics,
            &presenter_shared
                .graphics_pipelines
                .point_cloud_graphics_pipeline
                .descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.draw_indirect_count(
            &self.visible_point_cloud_instances_buffer,
            mem::size_of::<u32>() as u64,
            &self.visible_point_cloud_instances_buffer,
            0,
            self.point_cloud_instance_buffer.high_water_mark(),
        );
        drop(point_cloud_span);

        // Render with SimpleGraphicsPipeline
        let simple_span = span!("record simple commands");
        builder.bind_graphics_pipeline(&presenter_shared.graphics_pipelines.simple_graphics_pipeline);
        self.push_descriptors(
            PipelineBindPoint::Graphics,
            &presenter_shared.graphics_pipelines.simple_graphics_pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        drop(simple_span);

        // Render with ImmediateRenderingPipeline
        self.append_immediate_rendering_commands(backend_shared, presenter_shared, &mut builder, immediate_rendering_frames)?;

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
        let mut queues = backend_shared.queue_scheduler.queues();
        queues.presentation_queue(*window_id).submit_for_rendering_complete(
            command_buffer,
            image_available_semaphore,
            &main_rendering_complete_semaphore,
        )?;
        drop(queues);
        drop(submit_span);

        Ok(())
    }

    /// Processes the [`Transaction`]s pushed to the frame.
    fn process_transactions(&mut self) -> base::Result<()> {
        use transactions::Event;
        let drain = self.transactions.drain(..).collect::<Vec<_>>();
        for transaction in drain {
            for event in transaction.process() {
                match event {
                    Event::RigidMesh(rigid_mesh) => self.process_rigid_mesh_event(rigid_mesh)?,
                    Event::RigidMeshInstance(rigid_mesh_instance_event) => {
                        self.process_rigid_mesh_instance_event(rigid_mesh_instance_event)?
                    }
                    Event::PointCloud(point_cloud) => self.process_point_cloud_event(point_cloud)?,
                    Event::PointCloudInstance(point_cloud_instance_event) => {
                        self.process_point_cloud_instance_event(point_cloud_instance_event)?
                    }
                    Event::Camera(camera_event) => self.process_camera_event(camera_event)?,
                    Event::CameraInstance(camera_instance_event) => self.process_camera_instance_event(camera_instance_event)?,
                    Event::SetMeshAttributeActive {
                        gpu_index_allocation,
                        is_active,
                    } => {
                        self.mesh_attributes_active_buffer
                            .set(&gpu_index_allocation, &if is_active { 1 } else { 0 })?;
                    }
                    Event::SetPointCloudAttributesActive {
                        gpu_index_allocation,
                        is_active,
                    } => self
                        .point_cloud_attributes_active_buffer
                        .set(&gpu_index_allocation, &if is_active { 1 } else { 0 })?,
                }
            }
        }
        Ok(())
    }

    /// Processes a [`rigid_mesh::Event`].
    fn process_rigid_mesh_event(&mut self, event: rigid_mesh::Event) -> base::Result<()> {
        use rigid_mesh::Event;
        match event {
            Event::Insert(rigid_mesh) => {
                self.rigid_mesh_buffer.set(
                    rigid_mesh.gpu_index_allocation(),
                    &shader_interface::RigidMesh {
                        mesh_attributes_index: rigid_mesh.mesh_attributes().gpu_index_allocation().index() as i32,
                        preferred_mesh_representation: (*rigid_mesh.preferred_mesh_representation()).into(),
                    },
                )?;
            }
            Event::Noop => {}
        }
        Ok(())
    }

    fn process_point_cloud_event(&mut self, event: point_cloud::Event) -> base::Result<()> {
        use point_cloud::Event;
        match event {
            Event::Insert(point_cloud) => {
                self.point_cloud_buffer.set(
                    point_cloud.gpu_index_allocation(),
                    &shader_interface::PointCloud {
                        point_cloud_attributes_index: point_cloud.point_cloud_attributes().gpu_index_allocation().index() as i32,
                    },
                )?;
            }
            Event::Noop => {}
        }
        Ok(())
    }

    /// Processes a [`rigid_mesh_instance::Event`].
    fn process_rigid_mesh_instance_event(&mut self, event: rigid_mesh_instance::Event) -> base::Result<()> {
        use rigid_mesh_instance::Event;
        match event {
            Event::Noop => {}
            Event::Insert(rigid_mesh_instance) => {
                self.rigid_mesh_instance_buffer.set(
                    rigid_mesh_instance.gpu_index_allocation(),
                    &shader_interface::RigidMeshInstance {
                        rigid_mesh_index: rigid_mesh_instance.rigid_mesh_gpu_index_allocation().index() as u64,
                        _padding: 0,
                        transform: *rigid_mesh_instance.transform(),
                    },
                )?;
            }
        }
        Ok(())
    }

    /// Processes a [`point_cloud_instance::Event`].
    fn process_point_cloud_instance_event(&mut self, event: point_cloud_instance::Event) -> base::Result<()> {
        use point_cloud_instance::Event;
        match event {
            Event::Noop => {}
            Event::Insert(point_cloud_instance) => {
                self.point_cloud_instance_buffer.set(
                    point_cloud_instance.gpu_index_allocation(),
                    &shader_interface::PointCloudInstance {
                        point_cloud_index: point_cloud_instance.point_cloud_gpu_index_allocation().index() as u64,
                        _padding: 0,
                        transform: *point_cloud_instance.transform(),
                    },
                )?;
            }
        }
        Ok(())
    }

    /// Processes a [`camera::Event`].
    fn process_camera_event(&mut self, event: camera::Event) -> base::Result<()> {
        use camera::Event;
        match event {
            Event::Noop => {}
            Event::Insert(camera) => {
                info!("Insert Camera at {:?}", camera.gpu_index_allocation().index());
                self.camera_buffer.set(
                    camera.gpu_index_allocation(),
                    &shader_interface::Camera {
                        projection_matrix: camera.projection().projection_matrix(),
                    },
                )?;
            }
            Event::UpdateProjectionMatrix(gpu_index_allocation, matrix) => {
                self.camera_buffer
                    .set(&gpu_index_allocation, &shader_interface::Camera { projection_matrix: matrix })?;
            }
        }
        Ok(())
    }

    /// Processes a [`camera_instance::Event`].
    fn process_camera_instance_event(&mut self, event: camera_instance::Event) -> base::Result<()> {
        use camera_instance::Event;
        match event {
            Event::Noop => {}
            Event::Insert(camera_instance) => {
                info!("Insert CameraInstance at {:?}", camera_instance.gpu_index_allocation().index());
                self.camera_instance_buffer.set(
                    camera_instance.gpu_index_allocation(),
                    &shader_interface::CameraInstance {
                        camera_index: camera_instance.camera_gpu_index_allocation().index() as u64,
                        _padding: 0,
                        view_matrix: camera_instance.transform().view_matrix(),
                    },
                )?;
            }
            Event::UpdateViewMatrix(gpu_index_allocation, matrix) => {
                self.camera_instance_buffer.set(
                    &gpu_index_allocation,
                    &shader_interface::CameraInstance {
                        camera_index: gpu_index_allocation.index() as u64,
                        _padding: 0,
                        view_matrix: matrix,
                    },
                )?;
            }
        }
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
        let push_descriptors = &PushDescriptors::builder(descriptor_set_layout)
            .push_uniform_buffer(0, &self.per_frame_data_buffer)
            .push_storage_buffer(1, &self.camera_buffer)
            .push_storage_buffer(2, &self.camera_instance_buffer)
            .push_storage_buffer(3, &self.visible_rigid_mesh_instances_simple_buffer)
            .push_storage_buffer(5, &*backend_shared.static_vertex_position_buffer.lock())
            .push_storage_buffer(6, &*backend_shared.static_indices_buffer.lock())
            .push_storage_buffer(7, &*backend_shared.static_vertex_normals_buffer.lock())
            .push_storage_buffer(8, &*backend_shared.mesh_attributes_buffer.lock())
            .push_storage_buffer(9, &self.rigid_mesh_buffer)
            .push_storage_buffer(10, &self.mesh_attributes_active_buffer)
            .push_storage_buffer(11, &self.rigid_mesh_instance_buffer)
            .push_storage_buffer(12, &*backend_shared.static_meshlet_buffer.lock())
            .push_storage_buffer(13, &self.visible_rigid_mesh_instances)
            .push_storage_buffer(14, &self.visible_rigid_mesh_meshlets)
            .push_storage_buffer(15, &self.point_cloud_attributes_active_buffer)
            .push_storage_buffer(16, &self.point_cloud_buffer)
            .push_storage_buffer(17, &self.point_cloud_instance_buffer)
            .push_storage_buffer(18, &self.visible_point_cloud_instances_buffer)
            .push_storage_buffer(19, &*backend_shared.point_cloud_attributes_buffer.lock())
            .push_storage_buffer(20, &*backend_shared.static_point_positions_buffer.lock())
            .push_storage_buffer(21, &*backend_shared.static_point_colors_buffer.lock())
            .build();
        command_buffer_builder.push_descriptors(0, pipeline_bind_point, push_descriptors)?;
        Ok(())
    }

    fn append_immediate_rendering_commands(
        &self,
        backend_shared: &BackendShared,
        presenter_shared: &PresenterShared,
        command_buffer_builder: &mut CommandBufferBuilder,
        immediate_rendering_frames: &BTreeMap<&'static str, ImmediateRenderingFrameTask>,
    ) -> base::Result<()> {
        if immediate_rendering_frames.is_empty() {
            return Ok(());
        }

        // Collect vertex attributes for all immediate rendering requests
        let mut data = Vec::new();
        for task in immediate_rendering_frames.values() {
            for command_buffer in &task.immediate_command_buffer_handlers {
                for command in &command_buffer.commands {
                    match command {
                        ImmediateCommand::Matrix(..) => {}
                        ImmediateCommand::LineList(line_list) => data.extend_from_slice(line_list.positions()),
                        ImmediateCommand::LineStrip(line_strip) => data.extend_from_slice(line_strip.positions()),
                        ImmediateCommand::TriangleList(triangle_list) => data.extend_from_slice(triangle_list.positions()),
                        ImmediateCommand::TriangleStrip(triangle_strip) => data.extend_from_slice(triangle_strip.positions()),
                    }
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
            "immediate_rendering_commands_on_presenter_",
            self.presenter_index,
            immediate_rendering_frames
                .values()
                .flat_map(|task| &task.immediate_command_buffer_handlers)
                .flat_map(|command_buffer| &command_buffer.commands)
                .count() as f64
        );

        // Append the draw commands
        let mut first_vertex = 0;
        let mut last_matrix = Matrix4::identity();
        for task in immediate_rendering_frames.values() {
            for command_buffer in &task.immediate_command_buffer_handlers {
                let mut last_topology = None;
                for command in &command_buffer.commands {
                    match command {
                        ImmediateCommand::Matrix(matrix) => last_matrix = *matrix,
                        ImmediateCommand::LineList(line_list) => {
                            if !matches!(last_topology, Some(PrimitiveTopology::LineList)) {
                                command_buffer_builder
                                    .bind_graphics_pipeline(&presenter_shared.graphics_pipelines.immediate_graphics_pipeline_line_list);
                                self.push_descriptors(
                                    PipelineBindPoint::Graphics,
                                    &presenter_shared
                                        .graphics_pipelines
                                        .immediate_graphics_pipeline_line_list
                                        .descriptor_set_layout,
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
                                command_buffer_builder
                                    .bind_graphics_pipeline(&presenter_shared.graphics_pipelines.immediate_graphics_pipeline_line_strip);
                                self.push_descriptors(
                                    PipelineBindPoint::Graphics,
                                    &presenter_shared
                                        .graphics_pipelines
                                        .immediate_graphics_pipeline_line_strip
                                        .descriptor_set_layout,
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
                                command_buffer_builder
                                    .bind_graphics_pipeline(&presenter_shared.graphics_pipelines.immediate_graphics_pipeline_triangle_list);
                                self.push_descriptors(
                                    PipelineBindPoint::Graphics,
                                    &presenter_shared
                                        .graphics_pipelines
                                        .immediate_graphics_pipeline_triangle_list
                                        .descriptor_set_layout,
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
                                command_buffer_builder.bind_graphics_pipeline(
                                    &presenter_shared.graphics_pipelines.immediate_graphics_pipeline_triangle_strip,
                                );
                                self.push_descriptors(
                                    PipelineBindPoint::Graphics,
                                    &presenter_shared
                                        .graphics_pipelines
                                        .immediate_graphics_pipeline_triangle_strip
                                        .descriptor_set_layout,
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
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;

    use base::device::TestFixtureDevice;
    use jeriya_backend::{elements::camera::Camera, gpu_index_allocator::GpuIndexAllocation, transactions::PushEvent};
    use jeriya_shared::Handle;

    use super::*;

    #[test]
    fn resources() {
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let (resource_sender, _resource_receiver) = channel();
        let backend_shared = BackendShared::new(&test_fixture_device.device, &Arc::new(Default::default()), resource_sender).unwrap();
        let mut frame = Frame::new(0, &test_fixture_device.window.id(), &backend_shared).unwrap();
        let mut transaction = Transaction::new();
        let camera = Camera::new(
            camera::CameraProjection::Orthographic {
                left: -10.0,
                right: 5.0,
                bottom: 2.0,
                top: -3.0,
                near: 4.0,
                far: 11.0,
            },
            debug_info!("my_camera"),
            Handle::zero(),
            GpuIndexAllocation::new_unchecked(0),
        );
        transaction.push_event(transactions::Event::Camera(camera::Event::Insert(camera.clone())));
        frame.push_transaction(transaction);
        frame.process_transactions().unwrap();
        let mut data = vec![shader_interface::Camera::default(); frame.camera_buffer.capacity()];
        frame.camera_buffer.host_visible_buffer().get_memory_unaligned(&mut data).unwrap();
        assert_eq!(data[0].projection_matrix, camera.projection().projection_matrix());
    }
}
