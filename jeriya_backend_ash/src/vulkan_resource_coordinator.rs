use std::sync::Arc;

use base::graphics_pipeline::GenericGraphicsPipelineConfig;
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    device::Device, graphics_pipeline::GenericGraphicsPipeline, swapchain::Swapchain, swapchain_depth_buffer::SwapchainDepthBuffers,
    swapchain_framebuffers::SwapchainFramebuffers, swapchain_render_pass::SwapchainRenderPass,
};
use jeriya_shared::petgraph::Graph;

use crate::vulkan_resource_preparer::VulkanResourcePreparer;

/// Responsible for creating vulkan resources and managing their dependencies.
pub struct VulkanResourceCoordinator {
    device: Arc<Device>,

    graph: Graph<Node, ()>,

    swapchain_depth_buffers: SwapchainDepthBuffers,
    swapchain_framebuffers: SwapchainFramebuffers,
    swapchain_render_pass: SwapchainRenderPass,
}

impl VulkanResourceCoordinator {
    pub fn new(device: &Arc<Device>, preparer: &VulkanResourcePreparer, swapchain: &Swapchain) -> jeriya_backend::Result<Self> {
        let swapchain_depth_buffers = SwapchainDepthBuffers::new(device, &swapchain)?;
        let swapchain_render_pass = SwapchainRenderPass::new(device, &swapchain)?;
        let swapchain_framebuffers = SwapchainFramebuffers::new(device, &swapchain, &swapchain_depth_buffers, &swapchain_render_pass)?;

        Ok(VulkanResourceCoordinator {
            device: device.clone(),
            graph: Graph::new(),
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

    pub fn query_graphics_pipeline(&self, config: GenericGraphicsPipelineConfig) -> Option<Arc<GenericGraphicsPipeline>> {
        todo!()
    }

    pub fn query_compute_pipeline(&self) -> Option<Arc<GenericGraphicsPipeline>> {
        todo!()
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
