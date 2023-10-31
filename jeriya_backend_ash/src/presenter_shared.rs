use std::sync::Arc;

use base::specialization_constants::SpecializationConstants;
use jeriya_backend::{gpu_index_allocator::GpuIndexAllocation, instances::camera_instance::CameraInstance};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    compute_pipeline::{GenericComputePipeline, GenericComputePipelineConfig},
    device::Device,
    frame_index::FrameIndex,
    graphics_pipeline::{GenericGraphicsPipeline, GenericGraphicsPipelineConfig, GraphicsPipelineInterface, PrimitiveTopology},
    surface::Surface,
    swapchain::Swapchain,
    swapchain_depth_buffer::SwapchainDepthBuffers,
    swapchain_framebuffers::SwapchainFramebuffers,
    swapchain_render_pass::SwapchainRenderPass,
};
use jeriya_shared::{
    debug_info,
    log::info,
    nalgebra::{Matrix4, Vector4},
    winit::window::WindowId,
};

use crate::backend_shared::BackendShared;

pub struct IndirectGraphicsPipelineInterface;
impl GraphicsPipelineInterface for IndirectGraphicsPipelineInterface {
    type PushConstants = u32;
}

pub struct SimpleGraphicsPipelineInterface;
impl GraphicsPipelineInterface for SimpleGraphicsPipelineInterface {
    type PushConstants = u32;
}

pub struct PointCloudGraphicsPipelineInterface;
impl GraphicsPipelineInterface for PointCloudGraphicsPipelineInterface {
    type PushConstants = u32;
}

#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct PushConstants {
    pub color: Vector4<f32>,
    pub matrix: Matrix4<f32>,
}

pub struct ImmediateGraphicsPipelineInterface;
impl GraphicsPipelineInterface for ImmediateGraphicsPipelineInterface {
    type PushConstants = PushConstants;
}

pub struct GraphicsPipelines {
    pub simple_graphics_pipeline: GenericGraphicsPipeline<SimpleGraphicsPipelineInterface>,
    pub immediate_graphics_pipeline_line_list: GenericGraphicsPipeline<ImmediateGraphicsPipelineInterface>,
    pub immediate_graphics_pipeline_line_strip: GenericGraphicsPipeline<ImmediateGraphicsPipelineInterface>,
    pub immediate_graphics_pipeline_triangle_list: GenericGraphicsPipeline<ImmediateGraphicsPipelineInterface>,
    pub immediate_graphics_pipeline_triangle_strip: GenericGraphicsPipeline<ImmediateGraphicsPipelineInterface>,
    pub cull_rigid_mesh_instances_compute_pipeline: GenericComputePipeline,
    pub cull_rigid_mesh_meshlets_compute_pipeline: GenericComputePipeline,
    pub indirect_simple_graphics_pipeline: GenericGraphicsPipeline<IndirectGraphicsPipelineInterface>,
    pub indirect_meshlet_graphics_pipeline: GenericGraphicsPipeline<IndirectGraphicsPipelineInterface>,
    pub point_cloud_graphics_pipeline: GenericGraphicsPipeline<SimpleGraphicsPipelineInterface>,
}

impl GraphicsPipelines {
    fn new(
        device: &Arc<Device>,
        window_id: &WindowId,
        swapchain: &Swapchain,
        swapchain_render_pass: &SwapchainRenderPass,
    ) -> base::Result<Self> {
        macro_rules! spirv {
            ($shader:literal) => {
                Arc::new(include_bytes!(concat!("../../jeriya_backend_ash_base/test_data/", $shader)).to_vec())
            };
        }

        info!("Creating specialization constants");
        let specialization_constants = {
            let mut specialization_constants = SpecializationConstants::new();
            specialization_constants.push(0, 16);
            specialization_constants.push(1, 64);
            specialization_constants.push(2, 1024);
            specialization_constants.push(3, 1024);
            specialization_constants.push(4, 1024);
            specialization_constants.push(5, 1024);
            specialization_constants.push(6, 1048576);
            specialization_constants.push(7, 1024);
            specialization_constants.push(8, 1048576);
            specialization_constants.push(9, 1024);
            specialization_constants.push(10, 1024);
            specialization_constants
        };

        info!("Create Simple Graphics Pipeline");
        let simple_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("red_triangle.vert.spv")),
                fragment_shader_spirv: Some(spirv!("red_triangle.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                debug_info: debug_info!(format!("Simple-GraphicsPipeline-for-Window{:?}", window_id)),
                ..Default::default()
            };
            GenericGraphicsPipeline::new(device, &config, swapchain_render_pass, swapchain, &specialization_constants)?
        };

        info!("Create Immediate Graphics Pipelines");
        let create_immediate_graphics_pipeline = |primitive_topology| {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("color.vert.spv")),
                fragment_shader_spirv: Some(spirv!("color.frag.spv")),
                primitive_topology,
                use_input_attributes: true,
                use_dynamic_state_line_width: true,
                debug_info: debug_info!(format!("Immediate-GraphicsPipeline-for-Window{:?}", window_id)),
                ..Default::default()
            };
            GenericGraphicsPipeline::new(device, &config, swapchain_render_pass, swapchain, &specialization_constants)
        };
        let immediate_graphics_pipeline_line_list = create_immediate_graphics_pipeline(PrimitiveTopology::LineList)?;
        let immediate_graphics_pipeline_line_strip = create_immediate_graphics_pipeline(PrimitiveTopology::LineStrip)?;
        let immediate_graphics_pipeline_triangle_list = create_immediate_graphics_pipeline(PrimitiveTopology::TriangleList)?;
        let immediate_graphics_pipeline_triangle_strip = create_immediate_graphics_pipeline(PrimitiveTopology::TriangleStrip)?;

        info!("Create Point Cloud Graphics Pipeline");
        let point_cloud_graphics_pipeline = GenericGraphicsPipeline::new(
            device,
            &GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("point_cloud.vert.spv")),
                fragment_shader_spirv: Some(spirv!("point_cloud.frag.spv")),
                primitive_topology: PrimitiveTopology::PointList,
                debug_info: debug_info!(format!("Point-Cloud-GraphicsPipeline-for-Window{:?}", window_id)),
                ..Default::default()
            },
            swapchain_render_pass,
            swapchain,
            &specialization_constants,
        )?;

        info!("Create Cull Rigid Mesh Instances Compute Pipeline");
        let cull_rigid_mesh_instances_compute_pipeline = GenericComputePipeline::new(
            device,
            &GenericComputePipelineConfig {
                shader_spirv: spirv!("cull_rigid_mesh_instances.comp.spv"),
                debug_info: debug_info!(format!("Cull-RigidMeshInstances-ComputePipeline-for-Window{:?}", window_id)),
            },
            &specialization_constants,
        )?;

        info!("Create Cull Rigid Mesh Meshlets Compute Pipeline");
        let cull_rigid_mesh_meshlets_compute_pipeline = GenericComputePipeline::new(
            device,
            &GenericComputePipelineConfig {
                shader_spirv: spirv!("cull_rigid_mesh_meshlets.comp.spv"),
                debug_info: debug_info!(format!("Cull-RigidMeshMeshlets-ComputePipeline-for-Window{:?}", window_id)),
            },
            &specialization_constants,
        )?;

        info!("Create Indirect Simple Graphics Pipeline");
        let indirect_simple_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("indirect_simple.vert.spv")),
                fragment_shader_spirv: Some(spirv!("indirect_simple.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                debug_info: debug_info!(format!("Indirect-Simple-GraphicsPipeline-for-Window{:?}", window_id)),
                ..Default::default()
            };
            GenericGraphicsPipeline::new(device, &config, swapchain_render_pass, swapchain, &specialization_constants)?
        };

        info!("Create Indirect Meshlet Graphics Pipeline");
        let indirect_meshlet_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("indirect_meshlet.vert.spv")),
                fragment_shader_spirv: Some(spirv!("indirect_meshlet.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                debug_info: debug_info!(format!("Indirect-Meshlet-GraphicsPipeline-for-Window{:?}", window_id)),
                ..Default::default()
            };
            GenericGraphicsPipeline::new(device, &config, swapchain_render_pass, swapchain, &specialization_constants)?
        };

        Ok(Self {
            simple_graphics_pipeline,
            immediate_graphics_pipeline_line_list,
            immediate_graphics_pipeline_line_strip,
            immediate_graphics_pipeline_triangle_list,
            immediate_graphics_pipeline_triangle_strip,
            cull_rigid_mesh_instances_compute_pipeline,
            cull_rigid_mesh_meshlets_compute_pipeline,
            indirect_simple_graphics_pipeline,
            indirect_meshlet_graphics_pipeline,
            point_cloud_graphics_pipeline,
        })
    }
}

/// All the state that is required for presenting to the [`Surface`]
pub struct PresenterShared {
    pub frame_index: FrameIndex,
    pub desired_swapchain_length: u32,
    pub surface: Arc<Surface>,
    pub swapchain: Swapchain,
    pub swapchain_depth_buffers: SwapchainDepthBuffers,
    pub swapchain_framebuffers: SwapchainFramebuffers,
    pub swapchain_render_pass: SwapchainRenderPass,
    pub graphics_pipelines: GraphicsPipelines,
    pub active_camera_instance: Option<GpuIndexAllocation<CameraInstance>>,
    pub device: Arc<Device>,
}

impl PresenterShared {
    /// Creates a new `Presenter` for the [`Surface`]
    pub fn new(window_id: &WindowId, backend_shared: &BackendShared, surface: &Arc<Surface>) -> jeriya_backend::Result<Self> {
        let desired_swapchain_length = backend_shared.renderer_config.default_desired_swapchain_length;
        let swapchain = Swapchain::new(&backend_shared.device, surface, desired_swapchain_length, None)?;
        let swapchain_depth_buffers = SwapchainDepthBuffers::new(&backend_shared.device, &swapchain)?;
        let swapchain_render_pass = SwapchainRenderPass::new(&backend_shared.device, &swapchain)?;
        let swapchain_framebuffers =
            SwapchainFramebuffers::new(&backend_shared.device, &swapchain, &swapchain_depth_buffers, &swapchain_render_pass)?;

        info!("Create Graphics Pipelines");
        let graphics_pipelines = GraphicsPipelines::new(&backend_shared.device, window_id, &swapchain, &swapchain_render_pass)?;

        Ok(Self {
            frame_index: FrameIndex::new(),
            desired_swapchain_length,
            surface: surface.clone(),
            swapchain,
            swapchain_depth_buffers,
            swapchain_framebuffers,
            swapchain_render_pass,
            graphics_pipelines,
            device: backend_shared.device.clone(),
            active_camera_instance: None,
        })
    }

    /// Creates the swapchain and all state that depends on it
    pub fn recreate(&mut self, window_id: &WindowId, backend_shared: &BackendShared) -> base::Result<()> {
        // Locking all the queues at once so that no thread can submit to any
        // queue while waiting for the device to be idle.
        let _lock = backend_shared.queue_scheduler.queues();

        self.device.wait_for_idle()?;
        self.swapchain = Swapchain::new(&self.device, &self.surface, self.desired_swapchain_length, Some(&self.swapchain))?;
        self.swapchain_depth_buffers = SwapchainDepthBuffers::new(&self.device, &self.swapchain)?;
        self.swapchain_render_pass = SwapchainRenderPass::new(&self.device, &self.swapchain)?;
        self.swapchain_framebuffers = SwapchainFramebuffers::new(
            &self.device,
            &self.swapchain,
            &self.swapchain_depth_buffers,
            &self.swapchain_render_pass,
        )?;

        self.graphics_pipelines = GraphicsPipelines::new(&backend_shared.device, window_id, &self.swapchain, &self.swapchain_render_pass)?;

        Ok(())
    }

    /// Currently used [`Swapchain`]
    pub fn swapchain(&self) -> &Swapchain {
        &self.swapchain
    }

    /// Currently used [`SwapchainFramebuffers`]
    pub fn framebuffers(&self) -> &SwapchainFramebuffers {
        &self.swapchain_framebuffers
    }

    /// Currently used [`SwapchainRenderPass`]
    pub fn render_pass(&self) -> &SwapchainRenderPass {
        &self.swapchain_render_pass
    }

    /// Currently used [`DepthBuffers`]
    pub fn depth_buffers(&self) -> &SwapchainDepthBuffers {
        &self.swapchain_depth_buffers
    }
}

#[cfg(test)]
mod tests {
    mod new {
        use std::{
            iter,
            sync::{mpsc, Arc},
        };

        use jeriya_backend_ash_base::{
            device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, queue_plan::QueuePlan, surface::Surface,
        };
        use jeriya_shared::RendererConfig;
        use jeriya_test::create_window;

        use crate::{backend_shared::BackendShared, presenter_shared::PresenterShared};

        #[test]
        fn smoke() {
            let window = create_window();
            let entry = Entry::new().unwrap();
            let instance = Instance::new(&entry, "my_application", false).unwrap();
            let surface = Surface::new(&entry, &instance, &window).unwrap();
            let physical_device = PhysicalDevice::new(&instance).unwrap();
            let queue_plan = QueuePlan::new(&instance, &physical_device, iter::once((&window.id(), &surface))).unwrap();
            let device = Device::new(physical_device, &instance, queue_plan).unwrap();
            let (resource_sender, _resource_receiver) = mpsc::channel();
            let backend_shared = BackendShared::new(&device, &Arc::new(RendererConfig::default()), resource_sender).unwrap();
            let _presenter = PresenterShared::new(&window.id(), &backend_shared, &surface).unwrap();
        }
    }
}
