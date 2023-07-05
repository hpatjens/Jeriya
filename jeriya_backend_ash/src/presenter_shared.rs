use std::sync::Arc;

use jeriya_backend_ash_base as base;
use jeriya_backend_ash_base::{
    device::Device, immediate_graphics_pipeline::ImmediateGraphicsPipeline, immediate_graphics_pipeline::Topology,
    simple_graphics_pipeline::SimpleGraphicsPipeline, surface::Surface, swapchain::Swapchain,
    swapchain_depth_buffer::SwapchainDepthBuffers, swapchain_framebuffers::SwapchainFramebuffers,
    swapchain_render_pass::SwapchainRenderPass,
};
use jeriya_shared::{debug_info, parking_lot::Mutex, winit::window::WindowId, CameraContainerGuard, Handle};

use crate::{backend_shared::BackendShared, ImmediateRenderingRequest};

/// All the state that is required for presenting to the [`Surface`]
pub struct PresenterShared {
    pub desired_swapchain_length: u32,
    pub surface: Arc<Surface>,
    pub swapchain: Swapchain,
    pub swapchain_depth_buffers: SwapchainDepthBuffers,
    pub swapchain_framebuffers: SwapchainFramebuffers,
    pub swapchain_render_pass: SwapchainRenderPass,
    pub immediate_rendering_requests: Vec<ImmediateRenderingRequest>,
    pub simple_graphics_pipeline: SimpleGraphicsPipeline,
    pub immediate_graphics_pipeline_line_list: ImmediateGraphicsPipeline,
    pub immediate_graphics_pipeline_line_strip: ImmediateGraphicsPipeline,
    pub immediate_graphics_pipeline_triangle_list: ImmediateGraphicsPipeline,
    pub immediate_graphics_pipeline_triangle_strip: ImmediateGraphicsPipeline,
    pub active_camera: Handle<jeriya_shared::Camera>,
    pub device: Arc<Device>,
}

impl PresenterShared {
    /// Creates a new `Presenter` for the [`Surface`]
    pub fn new(window_id: &WindowId, backend_shared: &BackendShared, surface: &Arc<Surface>) -> jeriya_shared::Result<Self> {
        let desired_swapchain_length = backend_shared.renderer_config.default_desired_swapchain_length;
        let swapchain = Swapchain::new(&backend_shared.device, surface, desired_swapchain_length, None)?;
        let swapchain_depth_buffers = SwapchainDepthBuffers::new(&backend_shared.device, &swapchain)?;
        let swapchain_render_pass = SwapchainRenderPass::new(&backend_shared.device, &swapchain)?;
        let swapchain_framebuffers =
            SwapchainFramebuffers::new(&backend_shared.device, &swapchain, &swapchain_depth_buffers, &swapchain_render_pass)?;

        // Graphics Pipeline
        let simple_graphics_pipeline = SimpleGraphicsPipeline::new(
            &backend_shared.device,
            &swapchain_render_pass,
            &swapchain,
            &backend_shared.renderer_config,
            debug_info!(format!("SimpleGraphicsPipeline-for-Window{:?}", window_id)),
        )?;
        let immediate_graphics_pipeline_line_list = ImmediateGraphicsPipeline::new(
            &backend_shared.device,
            &swapchain_render_pass,
            &swapchain,
            Topology::LineList,
            &backend_shared.renderer_config,
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
        )?;
        let immediate_graphics_pipeline_line_strip = ImmediateGraphicsPipeline::new(
            &backend_shared.device,
            &swapchain_render_pass,
            &swapchain,
            Topology::LineStrip,
            &backend_shared.renderer_config,
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
        )?;
        let immediate_graphics_pipeline_triangle_list = ImmediateGraphicsPipeline::new(
            &backend_shared.device,
            &swapchain_render_pass,
            &swapchain,
            Topology::TriangleList,
            &backend_shared.renderer_config,
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
        )?;
        let immediate_graphics_pipeline_triangle_strip = ImmediateGraphicsPipeline::new(
            &backend_shared.device,
            &swapchain_render_pass,
            &swapchain,
            Topology::TriangleStrip,
            &backend_shared.renderer_config,
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
        )?;

        // Create a camera for this window
        let mut guard = CameraContainerGuard::new(
            backend_shared.camera_event_queue.lock(),
            backend_shared.cameras.lock(),
            backend_shared.renderer_config.clone(),
        );
        let active_camera = guard.insert(jeriya_shared::Camera::default())?;
        drop(guard);

        Ok(Self {
            desired_swapchain_length,
            surface: surface.clone(),
            swapchain,
            swapchain_depth_buffers,
            swapchain_framebuffers,
            swapchain_render_pass,
            immediate_rendering_requests: Vec::new(),
            simple_graphics_pipeline,
            immediate_graphics_pipeline_line_list,
            immediate_graphics_pipeline_line_strip,
            immediate_graphics_pipeline_triangle_list,
            immediate_graphics_pipeline_triangle_strip,
            device: backend_shared.device.clone(),
            active_camera,
        })
    }

    /// Creates the swapchain and all state that depends on it
    pub fn recreate(&mut self) -> base::Result<()> {
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
        use std::{iter, sync::Arc};

        use jeriya_backend_ash_base::{
            device::Device, entry::Entry, instance::Instance, physical_device::PhysicalDevice, surface::Surface,
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
            let physical_device = PhysicalDevice::new(&instance, iter::once(&surface)).unwrap();
            let device = Device::new(physical_device, &instance).unwrap();
            let backend_shared = BackendShared::new(&device, &Arc::new(RendererConfig::default())).unwrap();
            let _presenter = PresenterShared::new(&window.id(), &backend_shared, &surface).unwrap();
        }
    }
}
