mod resources;

pub use resources::*;

use std::{marker::PhantomData, result};

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
    pub fn builder() -> RendererBuilder<B> {
        RendererBuilder { _phantom: PhantomData }
    }

    pub fn create_resource_container(&self) -> ResourceContainerBuilder {
        ResourceContainerBuilder::new()
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }
}

/// Builder type to create an instance of the [`Renderer`]
pub struct RendererBuilder<B>
where
    B: Backend,
{
    _phantom: PhantomData<B>,
}

impl<B> RendererBuilder<B>
where
    B: Backend,
{
    pub fn build(self) -> Result<Renderer<B>> {
        Ok(Renderer::new(B::new()))
    }
}
