use std::{collections::VecDeque, sync::Arc};

use jeriya_backend_ash_core as core;
use jeriya_backend_ash_core::{
    buffer::BufferUsageFlags, command_buffer::CommandBuffer, device::Device, frame_index::FrameIndex,
    host_visible_buffer::HostVisibleBuffer, immediate_graphics_pipeline::ImmediateGraphicsPipeline, immediate_graphics_pipeline::Topology,
    semaphore::Semaphore, simple_graphics_pipeline::SimpleGraphicsPipeline, surface::Surface, swapchain_vec::SwapchainVec,
};
use jeriya_shared::{debug_info, winit::window::WindowId, Camera};

use crate::presenter_resources::PresenterResources;

pub struct Presenter {
    frame_index: FrameIndex,
    frame_index_history: VecDeque<FrameIndex>,
    pub presenter_resources: PresenterResources,
    pub image_available_semaphore: SwapchainVec<Option<Arc<Semaphore>>>,
    pub rendering_complete_semaphores: SwapchainVec<Vec<Arc<Semaphore>>>,
    pub rendering_complete_command_buffers: SwapchainVec<Vec<Arc<CommandBuffer>>>,
    pub simple_graphics_pipeline: SimpleGraphicsPipeline,
    pub immediate_graphics_pipeline_line_list: ImmediateGraphicsPipeline,
    pub immediate_graphics_pipeline_line_strip: ImmediateGraphicsPipeline,
    pub immediate_graphics_pipeline_triangle_list: ImmediateGraphicsPipeline,
    pub immediate_graphics_pipeline_triangle_strip: ImmediateGraphicsPipeline,
    pub cameras_buffer: SwapchainVec<HostVisibleBuffer<Camera>>,
}

impl Presenter {
    pub fn new(device: &Arc<Device>, window_id: &WindowId, surface: &Arc<Surface>, desired_swapchain_length: u32) -> core::Result<Self> {
        let presenter_resources = PresenterResources::new(device, surface, desired_swapchain_length)?;
        let image_available_semaphore = SwapchainVec::new(presenter_resources.swapchain(), |_| Ok(None))?;
        let rendering_complete_semaphores = SwapchainVec::new(presenter_resources.swapchain(), |_| Ok(Vec::new()))?;
        let rendering_complete_command_buffer = SwapchainVec::new(presenter_resources.swapchain(), |_| Ok(Vec::new()))?;
        let frame_index = FrameIndex::new();

        // Graphics Pipeline
        let simple_graphics_pipeline = SimpleGraphicsPipeline::new(
            &device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("SimpleGraphicsPipeline-for-Window{:?}", window_id)),
        )?;
        let immediate_graphics_pipeline_line_list = ImmediateGraphicsPipeline::new(
            &device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
            Topology::LineList,
        )?;
        let immediate_graphics_pipeline_line_strip = ImmediateGraphicsPipeline::new(
            &device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
            Topology::LineStrip,
        )?;
        let immediate_graphics_pipeline_triangle_list = ImmediateGraphicsPipeline::new(
            &device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
            Topology::TriangleList,
        )?;
        let immediate_graphics_pipeline_triangle_strip = ImmediateGraphicsPipeline::new(
            &device,
            presenter_resources.render_pass(),
            presenter_resources.swapchain(),
            debug_info!(format!("ImmediateGraphicsPipeline-for-Window{:?}", window_id)),
            Topology::TriangleStrip,
        )?;

        let cameras_buffer = SwapchainVec::new(presenter_resources.swapchain(), |_| {
            Ok(HostVisibleBuffer::new(
                &device,
                &vec![Camera::default(); 16],
                BufferUsageFlags::STORAGE_BUFFER,
                debug_info!(format!("CamerasBuffer-for-Window{:?}", window_id)),
            )?)
        })?;

        Ok(Self {
            frame_index,
            presenter_resources,
            image_available_semaphore,
            rendering_complete_semaphores,
            rendering_complete_command_buffers: rendering_complete_command_buffer,
            frame_index_history: VecDeque::new(),
            simple_graphics_pipeline,
            immediate_graphics_pipeline_line_list,
            immediate_graphics_pipeline_line_strip,
            immediate_graphics_pipeline_triangle_list,
            immediate_graphics_pipeline_triangle_strip,
            cameras_buffer,
        })
    }

    /// Recreates the [`PresenterResources`] in case of a swapchain resize
    pub fn recreate(&mut self) -> core::Result<()> {
        self.presenter_resources.recreate()
    }

    /// Sets the given [`FrameIndex`] and appends the previous one to the history
    pub fn start_frame(&mut self, frame_index: FrameIndex) {
        self.frame_index_history.push_front(self.frame_index.clone());
        self.frame_index = frame_index;
        while self.frame_index_history.len() > self.presenter_resources.swapchain().len() - 1 {
            self.frame_index_history.pop_back();
        }
    }

    /// Returns the current [`FrameIndex`]
    pub fn frame_index(&self) -> FrameIndex {
        self.frame_index.clone()
    }

    /// Returns the [`FrameIndex`] of the oldest frame in the history
    #[allow(dead_code)]
    pub fn oldest_frame_index(&self) -> Option<FrameIndex> {
        self.frame_index_history.back().cloned()
    }
}
