use jeriya_shared::{
    winit::window::{Window, WindowId},
    RendererConfig,
};

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend {
    type BackendConfig: Default;

    /// Creates a new [`Backend`]
    fn new(renderer_config: RendererConfig, backend_config: Self::BackendConfig, windows: &[&Window]) -> jeriya_shared::Result<Self>
    where
        Self: Sized;

    /// Is called when a window is resized so that the backend can respond.
    fn handle_window_resized(&self, window_id: WindowId) -> jeriya_shared::Result<()>;
}
