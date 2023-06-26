use std::sync::Arc;

use nalgebra::Matrix4;

use crate::{
    immediate::{CommandBuffer, CommandBufferBuilder, LineList, LineStrip, TriangleList, TriangleStrip},
    winit::window::{Window, WindowId},
    AsDebugInfo, Camera, CameraContainerGuard, DebugInfo, Handle, RendererConfig,
};

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend: Sized {
    type BackendConfig: Default;

    type ImmediateCommandBufferBuilderHandler: ImmediateCommandBufferBuilderHandler<Backend = Self> + AsDebugInfo;
    type ImmediateCommandBufferHandler: AsDebugInfo;

    /// Creates a new [`Backend`]
    fn new(renderer_config: RendererConfig, backend_config: Self::BackendConfig, windows: &[&Window]) -> crate::Result<Self>
    where
        Self: Sized;

    /// Is called when a window is resized so that the backend can respond.
    fn handle_window_resized(&self, window_id: WindowId) -> crate::Result<()>;

    /// Is called when rendering is requested.
    fn handle_render_frame(&self) -> crate::Result<()>;

    /// Creates a new [`CommandBufferBuilder`]
    fn create_immediate_command_buffer_builder(&self, debug_info: DebugInfo) -> crate::Result<CommandBufferBuilder<Self>>;

    /// Renders the given [`CommandBuffer`] in the next frame
    fn render_immediate_command_buffer(&self, command_buffer: Arc<CommandBuffer<Self>>) -> crate::Result<()>;

    /// Returns a guard to the cameras.
    fn cameras(&self) -> CameraContainerGuard;

    /// Sets the active camera for the given window
    fn set_active_camera(&self, window_id: WindowId, handle: Handle<Camera>) -> crate::Result<()>;
}

pub trait ImmediateCommandBufferBuilderHandler: AsDebugInfo {
    type Backend: Backend;

    /// Create a new [`ImmediateCommandBufferBuilderHandler`]
    fn new(backend: &Self::Backend, debug_info: DebugInfo) -> crate::Result<Self>
    where
        Self: Sized;

    /// Sets the matrix to be used for the following draw calls
    fn matrix(&mut self, matrix: Matrix4<f32>) -> crate::Result<()>;

    /// Push one or more [`LineList`]s to the command buffer
    fn push_line_lists(&mut self, line_lists: &[LineList]) -> crate::Result<()>;

    /// Push one or more [`LineStrip`]s to the command buffer
    fn push_line_strips(&mut self, line_strips: &[LineStrip]) -> crate::Result<()>;

    /// Push one or more [`TriangleList`]s to the command buffer
    fn push_triangle_lists(&mut self, triangle_lists: &[TriangleList]) -> crate::Result<()>;

    /// Push one or more [`TriangleStrip`]s to the command buffer
    fn push_triangle_strips(&mut self, triangle_strips: &[TriangleStrip]) -> crate::Result<()>;

    /// Build the command buffer and submit it for rendering
    fn build(self) -> crate::Result<Arc<CommandBuffer<Self::Backend>>>;
}
