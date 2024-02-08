use std::{collections::HashMap, sync::Arc};

use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    compute_pipeline::{GenericComputePipeline, GenericComputePipelineConfig},
    device::Device,
    graphics_pipeline::GenericGraphicsPipeline,
    graphics_pipeline::GenericGraphicsPipelineConfig,
    specialization_constants::SpecializationConstants,
    swapchain::Swapchain,
    swapchain_depth_buffer::SwapchainDepthBuffers,
    swapchain_framebuffers::SwapchainFramebuffers,
    swapchain_render_pass::SwapchainRenderPass,
};
use jeriya_content::shader::ShaderAsset;
use jeriya_shared::{ahash, log::info, RendererConfig};
use jeriya_shared::{debug_info, Handle, IndexingContainer};

/// Responsible for creating vulkan resources and managing their dependencies.
pub struct VulkanResourceCoordinator {
    device: Arc<Device>,

    specialization_constants: SpecializationConstants,

    // TODO: These are currently not freed
    graphics_pipeline_mapping: ahash::HashMap<GenericGraphicsPipelineConfig, Handle<Arc<GenericGraphicsPipeline>>>,
    compute_pipelines_mapping: ahash::HashMap<GenericComputePipelineConfig, Handle<Arc<GenericComputePipeline>>>,

    graphics_pipelines: IndexingContainer<Arc<GenericGraphicsPipeline>>,
    compute_pipelines: IndexingContainer<Arc<GenericComputePipeline>>,

    swapchain_depth_buffers: SwapchainDepthBuffers,
    swapchain_framebuffers: SwapchainFramebuffers,
    swapchain_render_pass: SwapchainRenderPass,
}

impl VulkanResourceCoordinator {
    pub fn new(device: &Arc<Device>, swapchain: &Swapchain, renderer_config: &RendererConfig) -> jeriya_backend::Result<Self> {
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
            specialization_constants,
            graphics_pipeline_mapping: HashMap::default(),
            compute_pipelines_mapping: HashMap::default(),
            graphics_pipelines: IndexingContainer::new(),
            compute_pipelines: IndexingContainer::new(),
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

    pub fn update_shader(&mut self, shader_asset: Arc<ShaderAsset>) -> base::Result<()> {
        info!("Updating shader {}", shader_asset.name());
        Ok(())
    }

    pub fn query_graphics_pipeline(&mut self, config: &GenericGraphicsPipelineConfig) -> base::Result<Arc<GenericGraphicsPipeline>> {
        if self.graphics_pipeline_mapping.contains_key(config) {
            let handle = &self.graphics_pipeline_mapping[config];
            let pipeline = self
                .graphics_pipelines
                .get(handle)
                .expect("pipeline not found due to inconsistent mapping")
                .clone();
            Ok(pipeline)
        } else {
            let pipeline = Arc::new(GenericGraphicsPipeline::new(
                &self.device,
                config,
                &self.swapchain_render_pass,
                &self.specialization_constants,
                debug_info!("GenericGraphicsPipeline"),
            )?);
            let handle = self.graphics_pipelines.insert(pipeline.clone());
            self.graphics_pipeline_mapping.insert(config.clone(), handle);
            Ok(pipeline)
        }
    }

    pub fn query_compute_pipeline(&mut self, config: &GenericComputePipelineConfig) -> base::Result<Arc<GenericComputePipeline>> {
        if self.compute_pipelines_mapping.contains_key(config) {
            let handle = &self.compute_pipelines_mapping[config];
            let pipeline = self
                .compute_pipelines
                .get(handle)
                .expect("pipeline not found due to inconsistent mapping")
                .clone();
            Ok(pipeline)
        } else {
            let pipeline = Arc::new(GenericComputePipeline::new(
                &self.device,
                config,
                &self.specialization_constants,
                debug_info!("GenericComputePipeline"),
            )?);
            let handle = self.compute_pipelines.insert(pipeline.clone());
            self.compute_pipelines_mapping.insert(config.clone(), handle);
            Ok(pipeline)
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    use jeriya_backend_ash_base::{device::TestFixtureDevice, swapchain::Swapchain};

    #[test]
    fn smoke() {
        let test_fixture_device = TestFixtureDevice::new().unwrap();
        let swapchain = Swapchain::new(&test_fixture_device.device, &test_fixture_device.surface, 3, None).unwrap();
        let _vulkan_resource_coordinator =
            VulkanResourceCoordinator::new(&test_fixture_device.device, &swapchain, &RendererConfig::default()).unwrap();
    }
}
