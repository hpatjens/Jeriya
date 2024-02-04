use std::sync::Arc;

use jeriya_backend::{gpu_index_allocator::GpuIndexAllocation, instances::camera_instance::CameraInstance};
use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    device::Device, frame_index::FrameIndex, surface::Surface, swapchain::Swapchain, swapchain_depth_buffer::SwapchainDepthBuffers,
    swapchain_framebuffers::SwapchainFramebuffers, swapchain_render_pass::SwapchainRenderPass,
};
use jeriya_shared::{log::info, winit::window::WindowId};

use crate::backend_shared::BackendShared;
use crate::pipeline_factory::PipelineFactory;
use crate::vulkan_resource_coordinator::VulkanResourceCoordinator;

/// All the state that is required for presenting to the [`Surface`]
pub struct PresenterShared {
    pub window_id: WindowId,
    pub frame_index: FrameIndex,
    pub desired_swapchain_length: u32,
    pub surface: Arc<Surface>,
    pub swapchain: Swapchain,
    pub vulkan_resource_coordinator: VulkanResourceCoordinator,
    pub pipeline_factory: PipelineFactory,
    pub active_camera_instance: Option<GpuIndexAllocation<CameraInstance>>,
    pub device: Arc<Device>,
}

impl PresenterShared {
    /// Creates a new `Presenter` for the [`Surface`]
    pub fn new(window_id: &WindowId, backend_shared: &BackendShared, surface: &Arc<Surface>) -> jeriya_backend::Result<Self> {
        let desired_swapchain_length = backend_shared.renderer_config.default_desired_swapchain_length;
        let swapchain = Swapchain::new(&backend_shared.device, surface, desired_swapchain_length, None)?;

        let mut vulkan_resource_coordinator =
            VulkanResourceCoordinator::new(&backend_shared.device, &backend_shared.vulkan_resource_preparer, &swapchain)?;

        info!("Create Graphics Pipelines");
        let graphics_pipelines = PipelineFactory::new(&swapchain, &mut vulkan_resource_coordinator, &backend_shared.asset_importer)?;

        Ok(Self {
            window_id: window_id.clone(),
            frame_index: FrameIndex::new(),
            desired_swapchain_length,
            surface: surface.clone(),
            swapchain,
            vulkan_resource_coordinator,
            pipeline_factory: graphics_pipelines,
            active_camera_instance: None,
            device: backend_shared.device.clone(),
        })
    }

    pub fn pre_frame_update(&mut self) {
        self.pipeline_factory.pre_frame_update();
    }

    /// Creates the swapchain and all state that depends on it
    pub fn recreate(&mut self, window_id: &WindowId, backend_shared: &BackendShared) -> base::Result<()> {
        // Locking all the queues at once so that no thread can submit to any
        // queue while waiting for the device to be idle.
        let _lock = backend_shared.queue_scheduler.queues();

        self.device.wait_for_idle()?;
        self.swapchain = Swapchain::new(&self.device, &self.surface, self.desired_swapchain_length, Some(&self.swapchain))?;
        self.vulkan_resource_coordinator.recreate(&self.swapchain)?;

        self.pipeline_factory = PipelineFactory::new(
            &self.swapchain,
            &mut self.vulkan_resource_coordinator,
            &backend_shared.asset_importer,
        )?;

        Ok(())
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
        use jeriya_content::{asset_importer::AssetImporter, shader::ShaderAsset};
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
            let asset_importer = Arc::new(AssetImporter::default_from("../assets/processed").unwrap());
            let backend_shared =
                BackendShared::new(&device, &Arc::new(RendererConfig::default()), resource_sender, &asset_importer).unwrap();
            let _presenter = PresenterShared::new(&window.id(), &backend_shared, &surface).unwrap();
        }
    }
}
