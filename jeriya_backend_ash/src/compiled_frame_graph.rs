use std::{collections::BTreeMap, mem, sync::Arc};

use jeriya_backend::immediate::{self, ImmediateCommand};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    buffer::BufferUsageFlags,
    command_buffer::CommandBuffer,
    command_buffer_builder::CommandBufferBuilder,
    command_buffer_builder::PipelineBindPoint,
    compute_pipeline::{GenericComputePipeline, GenericComputePipelineConfig},
    graphics_pipeline::PrimitiveTopology,
    graphics_pipeline::{GenericGraphicsPipeline, GenericGraphicsPipelineConfig, PushConstants},
    host_visible_buffer::HostVisibleBuffer,
    shader_interface, DispatchIndirectCommand, DrawIndirectCommand,
};
use jeriya_content::common::AssetKey;
use jeriya_shared::{debug_info, nalgebra::Matrix4, plot_with_index, tracy_client::plot, winit::window::WindowId};

use crate::{
    ash_immediate::ImmediateRenderingFrameTask, backend_shared::BackendShared, persistent_frame_state::PersistentFrameState,
    presenter_shared::PresenterShared,
};

pub struct CompiledFrameGraph {
    command_buffer: Option<CommandBuffer>,

    simple_graphics_pipeline: Arc<GenericGraphicsPipeline>,
    immediate_graphics_pipeline_line_list: Arc<GenericGraphicsPipeline>,
    immediate_graphics_pipeline_line_strip: Arc<GenericGraphicsPipeline>,
    immediate_graphics_pipeline_triangle_list: Arc<GenericGraphicsPipeline>,
    immediate_graphics_pipeline_triangle_strip: Arc<GenericGraphicsPipeline>,
    indirect_simple_graphics_pipeline: Arc<GenericGraphicsPipeline>,
    indirect_meshlet_graphics_pipeline: Arc<GenericGraphicsPipeline>,
    point_cloud_graphics_pipeline: Arc<GenericGraphicsPipeline>,
    point_cloud_clusters_graphics_pipeline: Arc<GenericGraphicsPipeline>,
    device_local_debug_lines_pipeline: Arc<GenericGraphicsPipeline>,

    cull_rigid_mesh_instances_compute_pipeline: Arc<GenericComputePipeline>,
    cull_rigid_mesh_meshlets_compute_pipeline: Arc<GenericComputePipeline>,
    cull_point_cloud_instances_compute_pipeline: Arc<GenericComputePipeline>,
    cull_point_cloud_clusters_compute_pipeline: Arc<GenericComputePipeline>,
    frame_telemetry_compute_pipeline: Arc<GenericComputePipeline>,
}

impl CompiledFrameGraph {
    pub fn new(presenter_shared: &mut PresenterShared) -> jeriya_backend::Result<Self> {
        let graphics_pipeline_default = GenericGraphicsPipelineConfig {
            primitive_topology: PrimitiveTopology::TriangleList,
            framebuffer_width: presenter_shared.swapchain.extent().width,
            framebuffer_height: presenter_shared.swapchain.extent().height,
            ..Default::default()
        };

        let simple_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader: Some(AssetKey::new("shaders/red_triangle.vert")),
                fragment_shader: Some(AssetKey::new("shaders/red_triangle.frag")),
                primitive_topology: PrimitiveTopology::TriangleList,
                ..graphics_pipeline_default.clone()
            };
            presenter_shared.vulkan_resource_coordinator.query_graphics_pipeline(&config)?
        };

        let mut create_immediate_graphics_pipeline = |primitive_topology| -> base::Result<_> {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader: Some(AssetKey::new("shaders/color.vert")),
                fragment_shader: Some(AssetKey::new("shaders/color.frag")),
                primitive_topology,
                use_input_attributes: true,
                use_dynamic_state_line_width: true,
                ..graphics_pipeline_default.clone()
            };
            presenter_shared.vulkan_resource_coordinator.query_graphics_pipeline(&config)
        };
        let immediate_graphics_pipeline_line_list = create_immediate_graphics_pipeline(PrimitiveTopology::LineList)?;
        let immediate_graphics_pipeline_line_strip = create_immediate_graphics_pipeline(PrimitiveTopology::LineStrip)?;
        let immediate_graphics_pipeline_triangle_list = create_immediate_graphics_pipeline(PrimitiveTopology::TriangleList)?;
        let immediate_graphics_pipeline_triangle_strip = create_immediate_graphics_pipeline(PrimitiveTopology::TriangleStrip)?;

        let point_cloud_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader: Some(AssetKey::new("shaders/point_cloud.vert")),
                fragment_shader: Some(AssetKey::new("shaders/point_cloud.frag")),
                primitive_topology: PrimitiveTopology::TriangleList,
                ..graphics_pipeline_default.clone()
            };
            presenter_shared.vulkan_resource_coordinator.query_graphics_pipeline(&config)?
        };

        let cull_point_cloud_instances_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader: AssetKey::new("shaders/cull_point_cloud_instances.comp"),
            };
            presenter_shared.vulkan_resource_coordinator.query_compute_pipeline(&config)?
        };

        let cull_point_cloud_clusters_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader: AssetKey::new("shaders/cull_point_cloud_clusters.comp"),
            };
            presenter_shared.vulkan_resource_coordinator.query_compute_pipeline(&config)?
        };

        let cull_rigid_mesh_instances_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader: AssetKey::new("shaders/cull_rigid_mesh_instances.comp"),
            };
            presenter_shared.vulkan_resource_coordinator.query_compute_pipeline(&config)?
        };

        let cull_rigid_mesh_meshlets_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader: AssetKey::new("shaders/cull_rigid_mesh_meshlets.comp"),
            };
            presenter_shared.vulkan_resource_coordinator.query_compute_pipeline(&config)?
        };

        let indirect_simple_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader: Some(AssetKey::new("shaders/indirect_simple.vert")),
                fragment_shader: Some(AssetKey::new("shaders/indirect_simple.frag")),
                primitive_topology: PrimitiveTopology::TriangleList,
                ..graphics_pipeline_default.clone()
            };
            presenter_shared.vulkan_resource_coordinator.query_graphics_pipeline(&config)?
        };

        let indirect_meshlet_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader: Some(AssetKey::new("shaders/indirect_meshlet.vert")),
                fragment_shader: Some(AssetKey::new("shaders/indirect_meshlet.frag")),
                primitive_topology: PrimitiveTopology::TriangleList,
                ..graphics_pipeline_default.clone()
            };
            presenter_shared.vulkan_resource_coordinator.query_graphics_pipeline(&config)?
        };

        let frame_telemetry_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader: AssetKey::new("shaders/frame_telemetry.comp"),
            };
            presenter_shared.vulkan_resource_coordinator.query_compute_pipeline(&config)?
        };

        let point_cloud_clusters_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader: Some(AssetKey::new("shaders/point_cloud_cluster.vert")),
                fragment_shader: Some(AssetKey::new("shaders/point_cloud_cluster.frag")),
                primitive_topology: PrimitiveTopology::TriangleList,
                ..graphics_pipeline_default.clone()
            };
            presenter_shared.vulkan_resource_coordinator.query_graphics_pipeline(&config)?
        };

        let device_local_debug_lines_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader: Some(AssetKey::new("shaders/device_local_debug_line.vert")),
                fragment_shader: Some(AssetKey::new("shaders/device_local_debug_line.frag")),
                primitive_topology: PrimitiveTopology::LineList,
                ..graphics_pipeline_default.clone()
            };
            presenter_shared.vulkan_resource_coordinator.query_graphics_pipeline(&config)?
        };

        Ok(CompiledFrameGraph {
            command_buffer: None,
            simple_graphics_pipeline,
            immediate_graphics_pipeline_line_list,
            immediate_graphics_pipeline_line_strip,
            immediate_graphics_pipeline_triangle_list,
            immediate_graphics_pipeline_triangle_strip,
            cull_rigid_mesh_instances_compute_pipeline,
            cull_rigid_mesh_meshlets_compute_pipeline,
            cull_point_cloud_instances_compute_pipeline,
            cull_point_cloud_clusters_compute_pipeline,
            frame_telemetry_compute_pipeline,
            indirect_simple_graphics_pipeline,
            indirect_meshlet_graphics_pipeline,
            point_cloud_graphics_pipeline,
            point_cloud_clusters_graphics_pipeline,
            device_local_debug_lines_pipeline,
        })
    }

    pub fn execute(
        &mut self,
        persistent_frame_state: &mut PersistentFrameState,
        window_id: &WindowId,
        backend_shared: &BackendShared,
        presenter_shared: &mut PresenterShared,
        immediate_rendering_frames: &BTreeMap<&'static str, ImmediateRenderingFrameTask>,
    ) -> jeriya_backend::Result<()> {
        let mut command_buffer = CommandBuffer::new(
            &backend_shared.device,
            &persistent_frame_state.command_pool,
            debug_info!("CommandBuffer-for-Swapchain-Renderpass"),
        )?;

        // Update Buffers
        let span = jeriya_shared::span!("update per frame data buffer");
        let per_frame_data = shader_interface::PerFrameData {
            active_camera: presenter_shared.active_camera_instance.map(|c| c.index() as i32).unwrap_or(-1),
            mesh_attributes_count: persistent_frame_state.mesh_attributes_active_buffer.high_water_mark() as u32,
            rigid_mesh_count: persistent_frame_state.rigid_mesh_buffer.high_water_mark() as u32,
            rigid_mesh_instance_count: persistent_frame_state.rigid_mesh_instance_buffer.high_water_mark() as u32,
            point_cloud_instance_count: persistent_frame_state.point_cloud_instance_buffer.high_water_mark() as u32,
            framebuffer_width: presenter_shared.swapchain.extent().width,
            framebuffer_height: presenter_shared.swapchain.extent().height,
        };
        persistent_frame_state
            .per_frame_data_buffer
            .set_memory_unaligned(&[per_frame_data])?;
        drop(span);

        const LOCAL_SIZE_X: u32 = 128;
        let cull_compute_shader_group_count =
            (persistent_frame_state.rigid_mesh_instance_buffer.high_water_mark() as u32 + LOCAL_SIZE_X - 1) / LOCAL_SIZE_X;

        // Build CommandBuffer
        let command_buffer_span = jeriya_shared::span!("build command buffer");
        let mut builder = CommandBufferBuilder::new(&backend_shared.device, &mut command_buffer)?;

        let begin_span = jeriya_shared::span!("begin command buffer");
        builder.begin_command_buffer_for_one_time_submit()?;
        drop(begin_span);

        // Wait for everything to be finished
        builder.bottom_to_top_pipeline_barrier();

        // Image transition to optimal layout
        builder.depth_pipeline_barrier(
            presenter_shared
                .vulkan_resource_coordinator
                .swapchain_depth_buffers()
                .depth_buffers
                .get(&presenter_shared.frame_index),
        )?;

        // Reset device local debug lines buffer
        let byte_size = mem::size_of::<u32>() as u64 + mem::size_of::<DrawIndirectCommand>() as u64;
        builder.fill_buffer(&persistent_frame_state.device_local_debug_lines_buffer, 0, byte_size, 0);
        builder.bottom_to_top_pipeline_barrier();

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
        let cull_rigid_mesh_instances_span = jeriya_shared::span!("cull rigid mesh instances");
        let pipeline = &self.cull_rigid_mesh_instances_compute_pipeline;
        builder.bind_compute_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Compute,
            &pipeline.descriptor_set_layout,
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
        builder.fill_buffer(&persistent_frame_state.visible_rigid_mesh_instances, 0, clear_bytes_count as u64, 0);

        // Clear counter for the visible rigid mesh instances that will be rendered with the
        // simple mesh representation.
        let clear_bytes_count = mem::size_of::<u32>();
        builder.transfer_to_transfer_command_barrier();
        builder.fill_buffer(
            &persistent_frame_state.visible_rigid_mesh_instances_simple_buffer,
            0,
            clear_bytes_count as u64,
            0,
        );

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
        let cull_meshlets_span = jeriya_shared::span!("cull meshlets");
        let pipeline = &self.cull_rigid_mesh_meshlets_compute_pipeline;
        builder.bind_compute_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Compute,
            &pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;

        // Clear counter for the visible meshlets
        builder.fill_buffer(
            &persistent_frame_state.visible_rigid_mesh_meshlets,
            0,
            mem::size_of::<u32>() as u64,
            0,
        );

        builder.transfer_to_indirect_command_barrier();
        builder.transfer_to_compute_pipeline_barrier();
        builder.compute_to_indirect_command_pipeline_barrier();
        builder.compute_to_compute_pipeline_barrier();

        // Dispatch compute shader for every visible rigid mesh instance
        builder.dispatch_indirect(&persistent_frame_state.visible_rigid_mesh_instances, 0);
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
        let cull_point_cloud_instances_span = jeriya_shared::span!("cull point cloud instances");
        let pipeline = &self.cull_point_cloud_instances_compute_pipeline;
        builder.bind_compute_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Compute,
            &pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;

        // Clear counter for the visible point cloud instances without clusters
        builder.fill_buffer(
            &persistent_frame_state.visible_point_cloud_instances_simple,
            0,
            mem::size_of::<u32>() as u64,
            0,
        );
        builder.transfer_to_compute_pipeline_barrier();

        // Clear counter for the visible point cloud instances with clusters
        let offset = mem::size_of::<DispatchIndirectCommand>() as u64;
        builder.fill_buffer(
            &persistent_frame_state.visible_point_cloud_instances,
            offset,
            mem::size_of::<u32>() as u64,
            0,
        );
        builder.transfer_to_compute_pipeline_barrier();

        // Dispatch
        let cull_point_cloud_instances_group_count = {
            const LOCAL_SIZE_X: u32 = 128;
            (persistent_frame_state.point_cloud_instance_buffer.high_water_mark() as u32 + LOCAL_SIZE_X - 1) / LOCAL_SIZE_X
        };
        builder.transfer_to_indirect_command_barrier();
        builder.transfer_to_compute_pipeline_barrier();
        builder.dispatch(cull_point_cloud_instances_group_count, 1, 1);

        builder.compute_to_indirect_command_pipeline_barrier();
        drop(cull_point_cloud_instances_span);

        // {
        //     let mut queues = backend_shared.queue_scheduler.queues();
        //     let buffer = self
        //         .visible_point_cloud_clusters
        //         .read_into_new_buffer_and_wait(queues.presentation_queue(*window_id), &command_pool)
        //         .unwrap();
        //     let count = buffer.get_memory_unaligned_index(0).unwrap();
        //     let offset = 4 * backend_shared.renderer_config.maximum_number_of_visible_point_cloud_clusters;
        //     let list = (0..32)
        //         .map(|i| buffer.get_memory_unaligned_index(offset + i).unwrap())
        //         .collect::<Vec<_>>();
        //     eprintln!("point_clouds: {count} -> {list:?}");
        // }

        let cull_point_cloud_clusters_span = jeriya_shared::span!("cull point cloud clusters");
        let pipeline = &self.cull_point_cloud_clusters_compute_pipeline;
        builder.bind_compute_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Compute,
            &pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;

        // Clear counter for the visible point cloud clusters
        builder.fill_buffer(
            &persistent_frame_state.visible_point_cloud_clusters,
            0,
            mem::size_of::<u32>() as u64,
            0,
        );

        // Dispatch
        builder.transfer_to_compute_pipeline_barrier();
        builder.transfer_to_indirect_command_barrier();
        builder.compute_to_indirect_command_pipeline_barrier();
        builder.compute_to_compute_pipeline_barrier();

        // Dispatch compute shader for culling the point cloud clusters
        builder.dispatch_indirect(&persistent_frame_state.visible_point_cloud_instances, 0);
        builder.compute_to_indirect_command_pipeline_barrier();

        drop(cull_point_cloud_clusters_span);

        // This barrier exists because the device local debug lines buffer is used
        // in the render pass. The barrier shouldn't be active in production code.
        builder.bottom_to_top_pipeline_barrier();

        // Render Pass
        let swapchain_image_index = presenter_shared
            .frame_index
            .swapchain_index()
            .expect("swapchain index must be set before rendering");
        builder.begin_render_pass(
            &presenter_shared.swapchain,
            presenter_shared.vulkan_resource_coordinator.swapchain_render_pass(),
            (
                presenter_shared.vulkan_resource_coordinator.swapchain_framebuffers(),
                swapchain_image_index,
            ),
        )?;

        // Render with IndirectSimpleGraphicsPipeline
        let indirect_simple_span = jeriya_shared::span!("record indirect simple commands");
        let pipeline = &self.indirect_simple_graphics_pipeline;
        builder.bind_graphics_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Graphics,
            &pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.draw_indirect_count(
            &persistent_frame_state.visible_rigid_mesh_instances_simple_buffer,
            mem::size_of::<u32>() as u64,
            &persistent_frame_state.visible_rigid_mesh_instances_simple_buffer,
            0,
            persistent_frame_state.rigid_mesh_instance_buffer.high_water_mark(),
        );
        drop(indirect_simple_span);

        // Render with IndirectMeshletGraphicsPipeline
        let indirect_meshlet_span = jeriya_shared::span!("record indirect meshlet commands");
        let pipeline = &self.indirect_meshlet_graphics_pipeline;
        builder.bind_graphics_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Graphics,
            &pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.draw_indirect_count(
            &persistent_frame_state.visible_rigid_mesh_meshlets,
            mem::size_of::<u32>() as u64,
            &persistent_frame_state.visible_rigid_mesh_meshlets,
            0,
            backend_shared.static_meshlet_buffer.lock().len(),
        );
        drop(indirect_meshlet_span);

        // Render Point Clouds
        let point_cloud_span = jeriya_shared::span!("record point cloud commands");
        let pipeline = &self.point_cloud_graphics_pipeline;
        builder.bind_graphics_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Graphics,
            &pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.draw_indirect_count(
            &persistent_frame_state.visible_point_cloud_instances_simple,
            mem::size_of::<u32>() as u64,
            &persistent_frame_state.visible_point_cloud_instances_simple,
            0,
            persistent_frame_state.point_cloud_instance_buffer.high_water_mark(),
        );
        drop(point_cloud_span);

        // Render with SimpleGraphicsPipeline
        let simple_span = jeriya_shared::span!("record simple commands");
        let pipeline = &self.simple_graphics_pipeline;
        builder.bind_graphics_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Graphics,
            &pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        drop(simple_span);

        // Render with PointCloudClusterGraphicsPipeline
        let indirect_meshlet_span = jeriya_shared::span!("record point cloud cluster commands");
        let pipeline = &self.point_cloud_clusters_graphics_pipeline;
        builder.bind_graphics_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Graphics,
            &pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.draw_indirect_count(
            &persistent_frame_state.visible_point_cloud_clusters,
            std::mem::size_of::<u32>() as u64,
            &persistent_frame_state.visible_point_cloud_clusters,
            0,
            backend_shared.renderer_config.maximum_number_of_visible_point_cloud_clusters,
        );
        drop(indirect_meshlet_span);

        // Render with ImmediateRenderingPipeline
        self.append_immediate_rendering_commands(persistent_frame_state, backend_shared, &mut builder, immediate_rendering_frames)?;

        // Render device local debug lines
        let device_local_debug_lines_span = jeriya_shared::span!("record device local debug lines commands");
        let pipeline = &self.device_local_debug_lines_pipeline;
        builder.bind_graphics_pipeline(pipeline.as_ref());
        persistent_frame_state.push_descriptors(
            PipelineBindPoint::Graphics,
            &pipeline.descriptor_set_layout,
            backend_shared,
            &mut builder,
        )?;
        builder.draw_indirect(
            &persistent_frame_state.device_local_debug_lines_buffer,
            mem::size_of::<u32>() as u64,
            1,
        );
        drop(device_local_debug_lines_span);

        builder.end_render_pass()?;

        // Write the frame telemetry data to the buffer
        let pipeline = &self.frame_telemetry_compute_pipeline;
        builder.bind_compute_pipeline(pipeline.as_ref());
        builder.bottom_to_top_pipeline_barrier();
        builder.dispatch(1, 1, 1);

        builder.end_command_buffer()?;

        drop(command_buffer_span);

        // Insert into Queue
        let submit_span = jeriya_shared::span!("submit command buffer commands");
        let mut queues = backend_shared.queue_scheduler.queues();
        queues.presentation_queue(*window_id).submit_for_rendering_complete(
            &command_buffer,
            &persistent_frame_state.image_available_semaphore,
            &persistent_frame_state.rendering_complete_semaphore,
            &persistent_frame_state.rendering_complete_fence,
        )?;
        drop(queues);
        drop(submit_span);

        self.command_buffer = Some(command_buffer);

        Ok(())
    }

    fn append_immediate_rendering_commands(
        &self,
        frame: &PersistentFrameState,
        backend_shared: &BackendShared,
        command_buffer_builder: &mut CommandBufferBuilder,
        immediate_rendering_frames: &BTreeMap<&'static str, ImmediateRenderingFrameTask>,
    ) -> base::Result<()> {
        if immediate_rendering_frames.is_empty() {
            return Ok(());
        }

        // Collect vertex attributes for all immediate rendering requests
        let mut data = Vec::new();
        for task in immediate_rendering_frames.values() {
            for command_buffer in &task.command_buffers {
                for command in command_buffer.commands() {
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
            frame.presenter_index,
            immediate_rendering_frames
                .values()
                .flat_map(|task| &task.command_buffers)
                .flat_map(immediate::CommandBuffer::commands)
                .count() as f64
        );

        // Append the draw commands
        let mut first_vertex = 0;
        let mut last_matrix = Matrix4::identity();
        for task in immediate_rendering_frames.values() {
            for command_buffer in &task.command_buffers {
                let mut last_topology = None;
                for command in command_buffer.commands() {
                    match command {
                        ImmediateCommand::Matrix(matrix) => last_matrix = *matrix,
                        ImmediateCommand::LineList(line_list) => {
                            if !matches!(last_topology, Some(PrimitiveTopology::LineList)) {
                                let pipeline = &self.immediate_graphics_pipeline_line_list;
                                command_buffer_builder.bind_graphics_pipeline(pipeline.as_ref());
                                frame.push_descriptors(
                                    PipelineBindPoint::Graphics,
                                    &pipeline.descriptor_set_layout,
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
                                let pipeline = &self.immediate_graphics_pipeline_line_strip;
                                command_buffer_builder.bind_graphics_pipeline(pipeline.as_ref());
                                frame.push_descriptors(
                                    PipelineBindPoint::Graphics,
                                    &pipeline.descriptor_set_layout,
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
                                let pipeline = &self.immediate_graphics_pipeline_triangle_list;
                                command_buffer_builder.bind_graphics_pipeline(pipeline.as_ref());
                                frame.push_descriptors(
                                    PipelineBindPoint::Graphics,
                                    &pipeline.descriptor_set_layout,
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
                                let pipeline = &self.immediate_graphics_pipeline_triangle_strip;
                                command_buffer_builder.bind_graphics_pipeline(pipeline.as_ref());
                                frame.push_descriptors(
                                    PipelineBindPoint::Graphics,
                                    &pipeline.descriptor_set_layout,
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
