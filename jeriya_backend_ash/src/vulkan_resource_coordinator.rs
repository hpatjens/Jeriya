use std::sync::Arc;

use base::compute_pipeline::{GenericComputePipeline, GenericComputePipelineConfig};
use base::graphics_pipeline::GenericGraphicsPipelineConfig;
use base::specialization_constants::SpecializationConstants;
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    device::Device, graphics_pipeline::GenericGraphicsPipeline, swapchain::Swapchain, swapchain_depth_buffer::SwapchainDepthBuffers,
    swapchain_framebuffers::SwapchainFramebuffers, swapchain_render_pass::SwapchainRenderPass,
};
use jeriya_shared::log::info;
use jeriya_shared::petgraph::Graph;
use jeriya_shared::{ahash, debug_info, RendererConfig};

use crate::vulkan_resource_preparer::VulkanResourcePreparer;

/// Responsible for creating vulkan resources and managing their dependencies.
pub struct VulkanResourceCoordinator {
    device: Arc<Device>,

    graph: Graph<Node, ()>,

    specialization_constants: SpecializationConstants,

    graphics_pipelines: ahash::HashMap<GenericGraphicsPipelineConfig, Arc<GenericGraphicsPipeline>>,
    compute_pipelines: ahash::HashMap<GenericComputePipelineConfig, Arc<GenericComputePipeline>>,

    swapchain_depth_buffers: SwapchainDepthBuffers,
    swapchain_framebuffers: SwapchainFramebuffers,
    swapchain_render_pass: SwapchainRenderPass,
}

impl VulkanResourceCoordinator {
    pub fn new(
        device: &Arc<Device>,
        preparer: &VulkanResourcePreparer,
        swapchain: &Swapchain,
        renderer_config: &RendererConfig,
    ) -> jeriya_backend::Result<Self> {
        info!("Creating swapchain resources");
        let swapchain_depth_buffers = SwapchainDepthBuffers::new(device, &swapchain)?;
        let swapchain_render_pass = SwapchainRenderPass::new(device, &swapchain)?;
        let swapchain_framebuffers = SwapchainFramebuffers::new(device, &swapchain, &swapchain_depth_buffers, &swapchain_render_pass)?;

        info!("Creating specialization constants");
        let specialization_constants = {
            let mut specialization_constants = SpecializationConstants::new();
            specialization_constants.push(0, renderer_config.maximum_number_of_cameras as u32);
            specialization_constants.push(1, renderer_config.maximum_number_of_camera_instances as u32);
            specialization_constants.push(2, renderer_config.maximum_number_of_point_cloud_attributes as u32);
            specialization_constants.push(3, renderer_config.maximum_number_of_rigid_meshes as u32);
            specialization_constants.push(4, renderer_config.maximum_number_of_mesh_attributes as u32);
            specialization_constants.push(5, renderer_config.maximum_number_of_rigid_mesh_instances as u32);
            specialization_constants.push(6, renderer_config.maximum_meshlets as u32);
            specialization_constants.push(7, renderer_config.maximum_visible_rigid_mesh_instances as u32);
            specialization_constants.push(8, renderer_config.maximum_visible_rigid_mesh_meshlets as u32);
            specialization_constants.push(9, renderer_config.maximum_number_of_point_clouds as u32);
            specialization_constants.push(10, renderer_config.maximum_number_of_point_cloud_instances as u32);
            specialization_constants.push(11, renderer_config.maximum_number_of_point_cloud_pages as u32);
            specialization_constants.push(12, 0);
            specialization_constants.push(13, 0);
            specialization_constants.push(14, renderer_config.maximum_number_of_visible_point_cloud_clusters as u32);
            specialization_constants.push(15, renderer_config.maximum_number_of_device_local_debug_lines as u32);
            specialization_constants
        };

        Ok(VulkanResourceCoordinator {
            device: device.clone(),
            graph: Graph::new(),
            specialization_constants,
            graphics_pipelines: ahash::HashMap::default(),
            compute_pipelines: ahash::HashMap::default(),
            swapchain_depth_buffers,
            swapchain_framebuffers,
            swapchain_render_pass,
        })
    }

    pub fn recreate(&mut self, swapchain: &Swapchain) -> base::Result<()> {
        self.swapchain_depth_buffers = SwapchainDepthBuffers::new(&self.device, swapchain)?;
        self.swapchain_render_pass = SwapchainRenderPass::new(&self.device, swapchain)?;
        self.swapchain_framebuffers =
            SwapchainFramebuffers::new(&self.device, swapchain, &self.swapchain_depth_buffers, &self.swapchain_render_pass)?;
        Ok(())
    }

    pub fn query_graphics_pipeline(&mut self, config: &GenericGraphicsPipelineConfig) -> base::Result<Arc<GenericGraphicsPipeline>> {
        if self.graphics_pipelines.contains_key(config) {
            Ok(self.graphics_pipelines[config].clone())
        } else {
            let pipeline = Arc::new(GenericGraphicsPipeline::new(
                &self.device,
                config,
                &self.swapchain_render_pass,
                &self.specialization_constants,
                debug_info!("GenericGraphicsPipeline"),
            )?);
            self.graphics_pipelines.insert(config.clone(), pipeline.clone());
            Ok(pipeline)
        }
    }

    pub fn query_compute_pipeline(&mut self, config: &GenericComputePipelineConfig) -> base::Result<Arc<GenericComputePipeline>> {
        if self.compute_pipelines.contains_key(config) {
            Ok(self.compute_pipelines[config].clone())
        } else {
            let pipeline = Arc::new(GenericComputePipeline::new(
                &self.device,
                config,
                &self.specialization_constants,
                debug_info!("GenericComputePipeline"),
            )?);
            self.compute_pipelines.insert(config.clone(), pipeline.clone());
            Ok(pipeline)
        }
    }

    pub fn query_swapchain_render_pass(&self) -> Option<Arc<SwapchainRenderPass>> {
        todo!()
    }

    pub fn swapchain_depth_buffers(&self) -> &SwapchainDepthBuffers {
        &self.swapchain_depth_buffers
    }

    pub fn swapchain_render_pass(&self) -> &SwapchainRenderPass {
        &self.swapchain_render_pass
    }

    pub fn swapchain_framebuffers(&self) -> &SwapchainFramebuffers {
        &self.swapchain_framebuffers
    }
}

pub struct Node {}

impl Node {
    pub fn new() -> Self {
        Node {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use jeriya_backend_ash_base::{device::TestFixtureDevice, swapchain::Swapchain};
    use jeriya_content::asset_importer::AssetImporter;

    use crate::vulkan_resource_preparer::VulkanResourcePreparer;

    #[test]
    fn smoke() {
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let swapchain = Swapchain::new(&test_fixture_device.device, &test_fixture_device.surface, 3, None).unwrap();
        let asset_importer = Arc::new(AssetImporter::default_from("../assets/processed").unwrap());
        let vulkan_resource_preparer = VulkanResourcePreparer::new(&asset_importer);
        let vulkan_resource_coordinator =
            VulkanResourceCoordinator::new(&test_fixture_device.device, &vulkan_resource_preparer, &swapchain);
    }
}
