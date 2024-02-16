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
use jeriya_content::asset_importer::{Asset, AssetImporter};
use jeriya_content::common::AssetKey;
use jeriya_content::shader::ShaderAsset;
use jeriya_shared::{ahash, log::info, RendererConfig};
use jeriya_shared::{debug_info, Handle, IndexingContainer};

/// Responsible for creating vulkan resources and managing their dependencies.
pub struct VulkanResourceCoordinator {
    device: Arc<Device>,

    asset_importer: Arc<AssetImporter>,

    specialization_constants: SpecializationConstants,

    // TODO: These are currently not freed
    graphics_pipeline_mapping: ahash::HashMap<GenericGraphicsPipelineConfig, Handle<Arc<GenericGraphicsPipeline>>>,
    compute_pipelines_mapping: ahash::HashMap<GenericComputePipelineConfig, Handle<Arc<GenericComputePipeline>>>,

    graphics_pipelines: IndexingContainer<Arc<GenericGraphicsPipeline>>,
    compute_pipelines: IndexingContainer<Arc<GenericComputePipeline>>,

    shader_asset_graphics_pipeline_mapping: ahash::HashMap<AssetKey, ahash::HashSet<Handle<Arc<GenericGraphicsPipeline>>>>,
    shader_asset_compute_pipeline_mapping: ahash::HashMap<AssetKey, ahash::HashSet<Handle<Arc<GenericComputePipeline>>>>,

    swapchain_depth_buffers: SwapchainDepthBuffers,
    swapchain_framebuffers: SwapchainFramebuffers,
    swapchain_render_pass: SwapchainRenderPass,
}

impl VulkanResourceCoordinator {
    pub fn new(
        device: &Arc<Device>,
        asset_importer: &Arc<AssetImporter>,
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
            asset_importer: asset_importer.clone(),
            specialization_constants,
            graphics_pipeline_mapping: HashMap::default(),
            compute_pipelines_mapping: HashMap::default(),
            graphics_pipelines: IndexingContainer::new(),
            compute_pipelines: IndexingContainer::new(),
            shader_asset_graphics_pipeline_mapping: HashMap::default(),
            shader_asset_compute_pipeline_mapping: HashMap::default(),
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

    pub fn update_shader(&mut self, shader_asset: Asset<ShaderAsset>) -> base::Result<()> {
        info!("Updating shader {}", shader_asset.asset_key().as_str());
        if let Some(graphics_pipeline_handles) = self.shader_asset_graphics_pipeline_mapping.get(shader_asset.asset_key()).cloned() {
            for handle in graphics_pipeline_handles.iter() {
                let old_pipeline_config = self
                    .graphics_pipelines
                    .get_mut(handle)
                    .expect("pipeline not found due to inconsistent mapping")
                    .config
                    .clone();
                self.try_build_graphics_pipeline(&old_pipeline_config)?;
            }
        }
        if let Some(compute_pipeline_handles) = self.shader_asset_compute_pipeline_mapping.get(shader_asset.asset_key()).cloned() {
            for handle in compute_pipeline_handles.iter() {
                let old_pipeline_config = self
                    .compute_pipelines
                    .get_mut(handle)
                    .expect("pipeline not found due to inconsistent mapping")
                    .config
                    .clone();
                self.try_build_compute_pipeline(&old_pipeline_config)?;
            }
        }
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
            self.try_build_graphics_pipeline(config)
        }
    }

    fn try_build_graphics_pipeline(&mut self, config: &GenericGraphicsPipelineConfig) -> base::Result<Arc<GenericGraphicsPipeline>> {
        let vertex_shader = config.vertex_shader.as_ref().expect("vertex shader not set");
        let fragment_shader = config.fragment_shader.as_ref().expect("fragment shader not set");
        let vertex_shader_spirv = if let Some(shader_asset) = self.asset_importer.get::<ShaderAsset>(vertex_shader) {
            shader_asset
                .value()
                .ok_or(base::Error::AssetNotFound {
                    asset_key: vertex_shader.clone(),
                    // This means that the asset was explicitly dropped after being imported
                    details: "Asset found via the `get` method but the value is None".to_owned(),
                })?
                .spriv()
                .to_vec()
        } else {
            self.asset_importer
                .import::<ShaderAsset>(vertex_shader)
                .map_err(|error| base::Error::AssetNotFound {
                    asset_key: vertex_shader.clone(),
                    details: format!(
                        "Asset not found via the get method. Starting and import if it's not already running. {}",
                        error
                    ),
                })?;
            return Err(base::Error::AssetNotFound {
                asset_key: vertex_shader.clone(),
                details: "Asset not found via the get method. Starting and import if it's not already running.".to_owned(),
            });
        };
        let fragment_shader_spirv = if let Some(shader_asset) = self.asset_importer.get::<ShaderAsset>(fragment_shader) {
            shader_asset
                .value()
                .ok_or(base::Error::AssetNotFound {
                    asset_key: fragment_shader.clone(),
                    // This means that the asset was explicitly dropped after being imported
                    details: "Asset found via the `get` method but the value is None".to_owned(),
                })?
                .spriv()
                .to_vec()
        } else {
            self.asset_importer
                .import::<ShaderAsset>(fragment_shader)
                .map_err(|error| base::Error::AssetNotFound {
                    asset_key: fragment_shader.clone(),
                    details: format!(
                        "Asset not found via the get method. Starting and import if it's not already running. {}",
                        error
                    ),
                })?;
            return Err(base::Error::AssetNotFound {
                asset_key: fragment_shader.clone(),
                details: "Asset not found via the get method. Starting and import if it's not already running.".to_owned(),
            });
        };
        let pipeline = Arc::new(GenericGraphicsPipeline::new(
            &self.device,
            config,
            &vertex_shader_spirv,
            &fragment_shader_spirv,
            &self.swapchain_render_pass,
            &self.specialization_constants,
            debug_info!("GenericGraphicsPipeline"),
        )?);
        let handle = self.graphics_pipelines.insert(pipeline.clone());
        self.graphics_pipeline_mapping.insert(config.clone(), handle);
        self.shader_asset_graphics_pipeline_mapping
            .entry(vertex_shader.clone())
            .or_insert_with(ahash::HashSet::default)
            .insert(handle);
        self.shader_asset_graphics_pipeline_mapping
            .entry(fragment_shader.clone())
            .or_insert_with(ahash::HashSet::default)
            .insert(handle);
        Ok(pipeline)
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
            self.try_build_compute_pipeline(config)
        }
    }

    fn try_build_compute_pipeline(&mut self, config: &GenericComputePipelineConfig) -> base::Result<Arc<GenericComputePipeline>> {
        let shader_spirv = if let Some(shader_asset) = self.asset_importer.get::<ShaderAsset>(&config.shader) {
            shader_asset
                .value()
                .ok_or(base::Error::AssetNotFound {
                    asset_key: config.shader.clone(),
                    // This means that the asset was explicitly dropped after being imported
                    details: "Asset found via the `get` method but the value is None".to_owned(),
                })?
                .spriv()
                .to_vec()
        } else {
            self.asset_importer
                .import::<ShaderAsset>(&config.shader)
                .map_err(|error| base::Error::AssetNotFound {
                    asset_key: config.shader.clone(),
                    details: format!(
                        "Asset not found via the get method. Starting and import if it's not already running. {}",
                        error
                    ),
                })?;
            return Err(base::Error::AssetNotFound {
                asset_key: config.shader.clone(),
                details: "Asset not found via the get method. Starting and import if it's not already running.".to_owned(),
            });
        };
        let pipeline = Arc::new(GenericComputePipeline::new(
            &self.device,
            config,
            &shader_spirv,
            &self.specialization_constants,
            debug_info!("GenericComputePipeline"),
        )?);
        let handle = self.compute_pipelines.insert(pipeline.clone());
        self.compute_pipelines_mapping.insert(config.clone(), handle);
        self.shader_asset_compute_pipeline_mapping
            .entry(config.shader.clone())
            .or_insert_with(ahash::HashSet::default)
            .insert(handle);
        Ok(pipeline)
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
        let asset_importer = Arc::new(AssetImporter::default_from("../assets/processed").unwrap());
        let _vulkan_resource_coordinator =
            VulkanResourceCoordinator::new(&test_fixture_device.device, &asset_importer, &swapchain, &RendererConfig::default()).unwrap();
    }
}
