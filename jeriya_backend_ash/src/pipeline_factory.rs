use std::sync::{mpsc::TryRecvError, Arc};

use base::specialization_constants::SpecializationConstants;
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    compute_pipeline::{GenericComputePipeline, GenericComputePipelineConfig},
    device::Device,
    graphics_pipeline::{GenericGraphicsPipeline, GenericGraphicsPipelineConfig, PrimitiveTopology},
    swapchain::Swapchain,
    swapchain_render_pass::SwapchainRenderPass,
};
use jeriya_content::asset_importer::{Asset, AssetImporter};
use jeriya_content::shader::ShaderAsset;
use jeriya_shared::{
    bus::BusReader,
    debug_info,
    log::{error, info},
    winit::window::WindowId,
    Handle, IndexingContainer,
};

use crate::vulkan_resource_coordinator::{self, VulkanResourceCoordinator};

pub struct PipelineFactory {
    pub simple_graphics_pipeline: Handle<Arc<GenericGraphicsPipeline>>,
    pub immediate_graphics_pipeline_line_list: Handle<Arc<GenericGraphicsPipeline>>,
    pub immediate_graphics_pipeline_line_strip: Handle<Arc<GenericGraphicsPipeline>>,
    pub immediate_graphics_pipeline_triangle_list: Handle<Arc<GenericGraphicsPipeline>>,
    pub immediate_graphics_pipeline_triangle_strip: Handle<Arc<GenericGraphicsPipeline>>,
    pub indirect_simple_graphics_pipeline: Handle<Arc<GenericGraphicsPipeline>>,
    pub indirect_meshlet_graphics_pipeline: Handle<Arc<GenericGraphicsPipeline>>,
    pub point_cloud_graphics_pipeline: Handle<Arc<GenericGraphicsPipeline>>,
    pub point_cloud_clusters_graphics_pipeline: Handle<Arc<GenericGraphicsPipeline>>,
    pub device_local_debug_lines_pipeline: Handle<Arc<GenericGraphicsPipeline>>,

    pub cull_rigid_mesh_instances_compute_pipeline: Handle<Arc<GenericComputePipeline>>,
    pub cull_rigid_mesh_meshlets_compute_pipeline: Handle<Arc<GenericComputePipeline>>,
    pub cull_point_cloud_instances_compute_pipeline: Handle<Arc<GenericComputePipeline>>,
    pub cull_point_cloud_clusters_compute_pipeline: Handle<Arc<GenericComputePipeline>>,
    pub frame_telemetry_compute_pipeline: Handle<Arc<GenericComputePipeline>>,

    shader_asset_receiver: BusReader<Arc<jeriya_content::Result<Asset<ShaderAsset>>>>,

    graphics_pipelines: IndexingContainer<Arc<GenericGraphicsPipeline>>,
    compute_pipelines: IndexingContainer<Arc<GenericComputePipeline>>,
}

impl PipelineFactory {
    pub fn new(
        swapchain: &Swapchain,
        vulkan_resource_coordinator: &mut VulkanResourceCoordinator,
        asset_importer: &Arc<AssetImporter>,
    ) -> base::Result<Self> {
        macro_rules! spirv {
            ($shader:literal) => {
                Arc::new(include_bytes!(concat!("../../jeriya_backend_ash_base/test_data/", $shader)).to_vec())
            };
        }

        let mut graphics_pipelines = IndexingContainer::new();
        let mut compute_pipelines = IndexingContainer::new();

        info!("Create Simple Graphics Pipeline");
        let simple_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("red_triangle.vert.spv")),
                fragment_shader_spirv: Some(spirv!("red_triangle.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                framebuffer_width: swapchain.extent().width,
                framebuffer_height: swapchain.extent().height,
                ..Default::default()
            };
            let pipeline = vulkan_resource_coordinator.query_graphics_pipeline(&config)?;
            graphics_pipelines.insert(pipeline)
        };

        info!("Create Immediate Graphics Pipelines");
        let mut create_immediate_graphics_pipeline = |primitive_topology| -> base::Result<_> {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("color.vert.spv")),
                fragment_shader_spirv: Some(spirv!("color.frag.spv")),
                primitive_topology,
                use_input_attributes: true,
                use_dynamic_state_line_width: true,
                framebuffer_width: swapchain.extent().width,
                framebuffer_height: swapchain.extent().height,
                ..Default::default()
            };
            let pipeline = vulkan_resource_coordinator.query_graphics_pipeline(&config)?;
            Ok(graphics_pipelines.insert(pipeline))
        };
        let immediate_graphics_pipeline_line_list = create_immediate_graphics_pipeline(PrimitiveTopology::LineList)?;
        let immediate_graphics_pipeline_line_strip = create_immediate_graphics_pipeline(PrimitiveTopology::LineStrip)?;
        let immediate_graphics_pipeline_triangle_list = create_immediate_graphics_pipeline(PrimitiveTopology::TriangleList)?;
        let immediate_graphics_pipeline_triangle_strip = create_immediate_graphics_pipeline(PrimitiveTopology::TriangleStrip)?;

        info!("Create Point Cloud Graphics Pipeline");
        let point_cloud_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("point_cloud.vert.spv")),
                fragment_shader_spirv: Some(spirv!("point_cloud.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                framebuffer_width: swapchain.extent().width,
                framebuffer_height: swapchain.extent().height,
                ..Default::default()
            };
            let pipeline = vulkan_resource_coordinator.query_graphics_pipeline(&config)?;
            graphics_pipelines.insert(pipeline)
        };

        info!("Create Cull Point Cloud Instances Compute Pipeline");
        let cull_point_cloud_instances_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader_spirv: spirv!("cull_point_cloud_instances.comp.spv"),
            };
            let pipeline = vulkan_resource_coordinator.query_compute_pipeline(&config)?;
            compute_pipelines.insert(pipeline)
        };

        info!("Create Cull Point Cloud Clusters Compute Pipeline");
        let cull_point_cloud_clusters_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader_spirv: spirv!("cull_point_cloud_clusters.comp.spv"),
            };
            let pipeline = vulkan_resource_coordinator.query_compute_pipeline(&config)?;
            compute_pipelines.insert(pipeline)
        };

        info!("Create Cull Rigid Mesh Instances Compute Pipeline");
        let cull_rigid_mesh_instances_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader_spirv: spirv!("cull_rigid_mesh_instances.comp.spv"),
            };
            let pipeline = vulkan_resource_coordinator.query_compute_pipeline(&config)?;
            compute_pipelines.insert(pipeline)
        };

        info!("Create Cull Rigid Mesh Meshlets Compute Pipeline");
        let cull_rigid_mesh_meshlets_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader_spirv: spirv!("cull_rigid_mesh_meshlets.comp.spv"),
            };
            let pipeline = vulkan_resource_coordinator.query_compute_pipeline(&config)?;
            compute_pipelines.insert(pipeline)
        };

        info!("Create Indirect Simple Graphics Pipeline");
        let indirect_simple_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("indirect_simple.vert.spv")),
                fragment_shader_spirv: Some(spirv!("indirect_simple.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                framebuffer_width: swapchain.extent().width,
                framebuffer_height: swapchain.extent().height,
                ..Default::default()
            };
            let pipeline = vulkan_resource_coordinator.query_graphics_pipeline(&config)?;
            graphics_pipelines.insert(pipeline)
        };

        info!("Create Indirect Meshlet Graphics Pipeline");
        let indirect_meshlet_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("indirect_meshlet.vert.spv")),
                fragment_shader_spirv: Some(spirv!("indirect_meshlet.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                framebuffer_width: swapchain.extent().width,
                framebuffer_height: swapchain.extent().height,
                ..Default::default()
            };
            let pipeline = vulkan_resource_coordinator.query_graphics_pipeline(&config)?;
            graphics_pipelines.insert(pipeline)
        };

        info!("Create Frame Telemetry Compute Pipeline");
        let frame_telemetry_compute_pipeline = {
            let config = GenericComputePipelineConfig {
                shader_spirv: spirv!("frame_telemetry.comp.spv"),
            };
            let pipeline = vulkan_resource_coordinator.query_compute_pipeline(&config)?;
            compute_pipelines.insert(pipeline)
        };

        info!("Create Point Cloud Clusters Graphics Pipeline");
        let point_cloud_clusters_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("point_cloud_cluster.vert.spv")),
                fragment_shader_spirv: Some(spirv!("point_cloud_cluster.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                framebuffer_width: swapchain.extent().width,
                framebuffer_height: swapchain.extent().height,
                ..Default::default()
            };
            let pipeline = vulkan_resource_coordinator.query_graphics_pipeline(&config)?;
            graphics_pipelines.insert(pipeline)
        };

        info!("Create Device Local Debug Line Pipeline");
        let device_local_debug_lines_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("device_local_debug_line.vert.spv")),
                fragment_shader_spirv: Some(spirv!("device_local_debug_line.frag.spv")),
                primitive_topology: PrimitiveTopology::LineList,
                framebuffer_width: swapchain.extent().width,
                framebuffer_height: swapchain.extent().height,
                ..Default::default()
            };
            let pipeline = vulkan_resource_coordinator.query_graphics_pipeline(&config)?;
            graphics_pipelines.insert(pipeline)
        };

        let shader_asset_receiver = asset_importer
            .receiver::<ShaderAsset>()
            .expect("Failed to get shader asset receiver");

        Ok(Self {
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
            shader_asset_receiver,
            graphics_pipelines,
            compute_pipelines,
        })
    }

    /// Returns the [`GenericGraphicsPipeline`] for the given [`Handle`]
    pub fn get_graphics_pipeline(&self, handle: &Handle<Arc<GenericGraphicsPipeline>>) -> &GenericGraphicsPipeline {
        self.graphics_pipelines.get(handle).expect("Invalid GraphicsPipeline handle")
    }

    /// Returns the [`GenericComputePipeline`] for the given [`Handle`]
    pub fn get_compute_pipeline(&self, handle: &Handle<Arc<GenericComputePipeline>>) -> &GenericComputePipeline {
        self.compute_pipelines.get(handle).expect("Invalid ComputePipeline handle")
    }

    pub fn pre_frame_update(&mut self) -> base::Result<()> {
        match self.shader_asset_receiver.try_recv() {
            Ok(import_result) => match import_result.as_ref() {
                Ok(asset) => {
                    let Some(shader_asset) = asset.value() else {
                        error!("Asset doesn't contain a shader asset");
                        return Err(base::Error::FailedToReceiveAsset(
                            "Receiver returned as error upon try_recv".to_string(),
                        ));
                    };

                    info!("Shader loaded");
                }
                Err(err) => error!("Failed to receive asset import result: {:?}", err),
            },
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                error!("Failed to receive asset import result");
                return Err(base::Error::FailedToReceiveAsset(
                    "Receiver returned as error upon try_recv".to_string(),
                ));
            }
        }
        Ok(())
    }
}
