mod resources;

use jeriya_shared::{
    winit::window::{Window, WindowId},
    RendererConfig, Result,
};
pub use resources::*;

use std::marker::PhantomData;

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend {
    type BackendConfig: Default;

    /// Creates a new [`Backend`]
    fn new(renderer_config: RendererConfig, backend_config: Self::BackendConfig, windows: &[&Window]) -> Result<Self>
    where
        Self: Sized;

    /// Is called when a window is resized so that the backend can respond.
    fn handle_window_resized(&self, window_id: WindowId) -> Result<()>;
}

/// Instance of the renderer
pub struct Renderer<B>
where
    B: Backend,
{
    backend: B,
}

impl<B> Renderer<B>
where
    B: Backend,
{
    fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Creates a new [`RendererBuilder`] to create an instance of the `Renderer`
    pub fn builder<'a>() -> RendererBuilder<'a, B> {
        RendererBuilder::new()
    }

    pub fn create_resource_container(&self) -> ResourceContainerBuilder {
        ResourceContainerBuilder::new()
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn render_frame(&self) {}

    /// Has to be called when a window is gets resized.
    pub fn window_resized(&self, window_id: WindowId) -> Result<()> {
        self.backend.handle_window_resized(window_id)
    }
}

/// Builder type to create an instance of the [`Renderer`]
pub struct RendererBuilder<'a, B>
where
    B: Backend,
{
    _phantom: PhantomData<B>,
    windows: &'a [&'a Window],
    renderer_config: Option<RendererConfig>,
    backend_config: Option<B::BackendConfig>,
}

impl<'a, B> RendererBuilder<'a, B>
where
    B: Backend,
{
    fn new() -> Self {
        Self {
            _phantom: PhantomData,
            windows: &[],
            renderer_config: None,
            backend_config: None,
        }
    }

    pub fn add_renderer_config(mut self, renderer_config: RendererConfig) -> Self {
        self.renderer_config = Some(renderer_config);
        self
    }

    pub fn add_backend_config(mut self, backend_config: B::BackendConfig) -> Self {
        self.backend_config = Some(backend_config);
        self
    }

    pub fn add_windows(mut self, windows: &'a [&'a Window]) -> Self {
        self.windows = windows;
        self
    }

    pub fn build(self) -> Result<Renderer<B>> {
        let renderer_config = self.renderer_config.unwrap_or(RendererConfig::default());
        let backend_config = self.backend_config.unwrap_or(B::BackendConfig::default());
        let backend = B::new(renderer_config, backend_config, self.windows)?;
        Ok(Renderer::new(backend))
    }
}
