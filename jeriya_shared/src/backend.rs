use std::sync::Arc;

use crate::{
    immediate::{CommandBufferConfig, Line},
    winit::window::{Window, WindowId},
    DebugInfo, RendererConfig,
};

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend {
    type BackendConfig: Default;

    type ImmediateRenderingBackend: ImmediateRenderingBackend<Backend = Self>;

    /// Creates a new [`Backend`]
    fn new(renderer_config: RendererConfig, backend_config: Self::BackendConfig, windows: &[&Window]) -> crate::Result<Self>
    where
        Self: Sized;

    /// Is called when a window is resized so that the backend can respond.
    fn handle_window_resized(&self, window_id: WindowId) -> crate::Result<()>;

    /// Is called when rendering is requested.
    fn handle_render_frame(&self) -> crate::Result<()>;

    /// Returns the [`ImmediateRenderingBackend`]
    fn immediate_rendering_backend(&self) -> &Self::ImmediateRenderingBackend;
}

/// Backend functionality for immediate mode rendering
pub trait ImmediateRenderingBackend {
    type Backend: Backend;

    type CommandBuffer;

    /// Is called when `CommandBufferBuilder::new` is called.
    fn handle_new(
        &self,
        backend: &Self::Backend,
        config: CommandBufferConfig,
        debug_info: DebugInfo,
    ) -> crate::Result<Arc<Self::CommandBuffer>>;

    /// Is called when `CommandBufferBuilder::set_config` is called
    fn handle_set_config(
        &self,
        backend: &Self::Backend,
        command_buffer: &Arc<Self::CommandBuffer>,
        config: CommandBufferConfig,
    ) -> crate::Result<()>;

    /// Is called when `CommandBufferBuilder::push_line` is called.
    fn handle_push_line(&self, backend: &Self::Backend, command_buffer: &Arc<Self::CommandBuffer>, line: Line) -> crate::Result<()>;

    /// Is called when `CommandBufferBuilder::build` is called.
    fn handle_build(&self, backend: &Self::Backend, command_buffer: &Arc<Self::CommandBuffer>) -> crate::Result<()>;
}
