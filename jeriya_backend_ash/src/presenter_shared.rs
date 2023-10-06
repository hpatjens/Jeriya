use std::sync::Arc;

use jeriya_backend::CameraContainerGuard;
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
use jeriya_shared::RendererConfig;
use jeriya_shared::{
    debug_info,
    log::info,
    nalgebra::{Matrix4, Vector4},
    winit::window::WindowId,
    Handle,
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
    pub cull_compute_pipeline: GenericComputePipeline,
    pub indirect_graphics_pipeline: GenericGraphicsPipeline<IndirectGraphicsPipelineInterface>,
}

impl GraphicsPipelines {
    fn new(
        device: &Arc<Device>,
        window_id: &WindowId,
        renderer_config: &RendererConfig,
        swapchain: &Swapchain,
        swapchain_render_pass: &SwapchainRenderPass,
    ) -> base::Result<Self> {
        macro_rules! spirv {
            ($shader:literal) => {
                Arc::new(include_bytes!(concat!("../../jeriya_backend_ash_base/test_data/", $shader)).to_vec())
            };
        }

        info!("Create Simple Graphics Pipeline");
        let simple_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("red_triangle.vert.spv")),
                fragment_shader_spirv: Some(spirv!("red_triangle.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                debug_info: debug_info!(format!("Simple-GraphicsPipeline-for-Window{:?}", window_id)),
                ..Default::default()
            };
            GenericGraphicsPipeline::new(&device, &config, &swapchain_render_pass, &swapchain, &renderer_config)?
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
            GenericGraphicsPipeline::new(&device, &config, &swapchain_render_pass, &swapchain, &renderer_config)
        };
        let immediate_graphics_pipeline_line_list = create_immediate_graphics_pipeline(PrimitiveTopology::LineList)?;
        let immediate_graphics_pipeline_line_strip = create_immediate_graphics_pipeline(PrimitiveTopology::LineStrip)?;
        let immediate_graphics_pipeline_triangle_list = create_immediate_graphics_pipeline(PrimitiveTopology::TriangleList)?;
        let immediate_graphics_pipeline_triangle_strip = create_immediate_graphics_pipeline(PrimitiveTopology::TriangleStrip)?;

        info!("Create Compute Pipeline");
        let cull_compute_pipeline = GenericComputePipeline::new(
            &device,
            &GenericComputePipelineConfig {
                shader_spirv: spirv!("cull.comp.spv"),
                debug_info: debug_info!(format!("Cull-ComputePipeline-for-Window{:?}", window_id)),
            },
        )?;

        info!("Create Indirect Graphics Pipeline");
        let indirect_graphics_pipeline = {
            let config = GenericGraphicsPipelineConfig {
                vertex_shader_spirv: Some(spirv!("indirect.vert.spv")),
                fragment_shader_spirv: Some(spirv!("indirect.frag.spv")),
                primitive_topology: PrimitiveTopology::TriangleList,
                debug_info: debug_info!(format!("Indirect-GraphicsPipeline-for-Window{:?}", window_id)),
                ..Default::default()
            };
            GenericGraphicsPipeline::new(&device, &config, &swapchain_render_pass, &swapchain, &renderer_config)?
        };

        Ok(Self {
            simple_graphics_pipeline,
            immediate_graphics_pipeline_line_list,
            immediate_graphics_pipeline_line_strip,
            immediate_graphics_pipeline_triangle_list,
            immediate_graphics_pipeline_triangle_strip,
            cull_compute_pipeline,
            indirect_graphics_pipeline,
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
    pub active_camera: Handle<jeriya_backend::Camera>,
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

        // Create a camera for this window
        info!("Create Camera");
        let mut guard = CameraContainerGuard::new(
            backend_shared.camera_event_queue.lock(),
            backend_shared.cameras.lock(),
            backend_shared.renderer_config.clone(),
        );
        let active_camera = guard.insert(jeriya_backend::Camera::default())?;
        drop(guard);

        info!("Create Graphics Pipelines");
        let graphics_pipelines = GraphicsPipelines::new(
            &backend_shared.device,
            window_id,
            &backend_shared.renderer_config,
            &swapchain,
            &swapchain_render_pass,
        )?;

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
            active_camera,
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

        self.graphics_pipelines = GraphicsPipelines::new(
            &backend_shared.device,
            window_id,
            &backend_shared.renderer_config,
            &self.swapchain,
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
