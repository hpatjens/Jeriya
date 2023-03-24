mod resources;

use jeriya_shared::{winit::window::Window, Result};
pub use resources::*;

use std::marker::PhantomData;

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend {
    fn new(application_name: Option<&str>, windows: &[&Window]) -> Result<Self>
    where
        Self: Sized;
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
}

/// Builder type to create an instance of the [`Renderer`]
pub struct RendererBuilder<'a, B>
where
    B: Backend,
{
    _phantom: PhantomData<B>,
    windows: &'a [&'a Window],
    application_name: Option<&'a str>,
}

impl<'a, B> RendererBuilder<'a, B>
where
    B: Backend,
{
    fn new() -> Self {
        Self {
            _phantom: PhantomData,
            windows: &[],
            application_name: None,
        }
    }

    pub fn add_application_name(mut self, application_name: &'a str) -> Self {
        self.application_name = Some(application_name);
        self
    }

    pub fn add_windows(mut self, windows: &'a [&'a Window]) -> Self {
        self.windows = windows;
        self
    }

    pub fn build(self) -> Result<Renderer<B>> {
        let backend = B::new(self.application_name, self.windows)?;
        Ok(Renderer::new(backend))
    }
}
