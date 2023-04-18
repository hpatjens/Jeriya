use crate::{
    immediate::{CommandBufferConfig, Line},
    winit::window::{Window, WindowId},
    AsDebugInfo, DebugInfo, RendererConfig,
};

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend {
    type BackendConfig: Default;

    type ImmediateCommandBufferBuilder: ImmediateCommandBufferBuilder<Backend = Self>;

    /// Creates a new [`Backend`]
    fn new(renderer_config: RendererConfig, backend_config: Self::BackendConfig, windows: &[&Window]) -> crate::Result<Self>
    where
        Self: Sized;

    /// Is called when a window is resized so that the backend can respond.
    fn handle_window_resized(&self, window_id: WindowId) -> crate::Result<()>;

    /// Is called when rendering is requested.
    fn handle_render_frame(&self) -> crate::Result<()>;

    /// Creates a new [`ImmediateCommandBufferBuilder`]
    fn create_immediate_command_buffer_builder(
        &self,
        config: CommandBufferConfig,
        debug_info: DebugInfo,
    ) -> crate::Result<Self::ImmediateCommandBufferBuilder>;
}

pub trait ImmediateCommandBufferBuilder: AsDebugInfo {
    type Backend: Backend;

    /// Create a new [`ImmediateCommandBufferBuilder`]
    fn new(backend: &Self::Backend, config: CommandBufferConfig, debug_info: DebugInfo) -> crate::Result<Self>
    where
        Self: Sized;

    /// Set the configuration of the command buffer
    fn set_config(&mut self, config: CommandBufferConfig) -> crate::Result<()>;

    /// Push a line to the command buffer
    fn push_line(&mut self, line: Line) -> crate::Result<()>;

    /// Build the command buffer and submit it for rendering
    fn build(self) -> crate::Result<()>;
}
