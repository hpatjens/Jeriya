mod resources;

pub use resources::*;
use winit::window::{Window, WindowId};

use std::{collections::HashMap, marker::PhantomData, result};

#[derive(Debug)]
pub enum Error {}

pub type Result<T> = result::Result<T, Error>;

/// Rendering backend that is used by the [`Renderer`]
pub trait Backend {
    fn new() -> Self
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
    windows: HashMap<WindowId, &'a Window>,
}

impl<'a, B> RendererBuilder<'a, B>
where
    B: Backend,
{
    fn new() -> Self {
        Self {
            _phantom: PhantomData,
            windows: HashMap::new(),
        }
    }

    pub fn add_windows(mut self, windows: &[&'a Window]) -> Self {
        self.windows.extend(windows.into_iter().map(|w| (w.id(), *w)));
        self
    }

    pub fn build(self) -> Result<Renderer<B>> {
        Ok(Renderer::new(B::new()))
    }
}
