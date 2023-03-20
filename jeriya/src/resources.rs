mod static_mesh;
mod texture2d;

pub use static_mesh::*;
pub use texture2d::*;

use std::sync::Arc;

use jeriya_shared::DebugInfo;
use parking_lot::Mutex;

/// Data on the GPU that doesn't change frequently and is referenced by the instances in the scene
pub trait Resource {
    fn new() -> Self
    where
        Self: Sized;
}

/// Collection of [`Resource`]s with a shared commonality
#[derive(Default)]
pub struct ResourceContainer {
    pub debug_info: Option<DebugInfo>,
    pub texture2ds: ResourceGroup<Texture2d>,
    pub static_meshes: ResourceGroup<StaticMesh>,
}

/// Builder for a [`ResourceContainer`]
pub struct ResourceContainerBuilder {
    debug_info: Option<DebugInfo>,
}

impl ResourceContainerBuilder {
    pub(crate) fn new() -> Self {
        Self { debug_info: None }
    }

    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    pub fn build(self) -> ResourceContainer {
        ResourceContainer {
            debug_info: self.debug_info,
            ..Default::default()
        }
    }
}

/// Collection of [`Resource`]s of the same type
#[derive(Default)]
pub struct ResourceGroup<R> {
    _data: Vec<R>,
}

impl ResourceGroup<Texture2d> {
    pub fn create(&self) -> ResourceBuilder<Texture2d> {
        ResourceBuilder::new(self)
    }
}

/// Builder for a [`Resource`]
pub struct ResourceBuilder<'resgr, R> {
    _resource_group: &'resgr ResourceGroup<R>,
    debug_info: Option<DebugInfo>,
}

impl<'resgr, R> ResourceBuilder<'resgr, R>
where
    R: Resource,
{
    fn new(resource_group: &'resgr ResourceGroup<R>) -> Self {
        Self {
            _resource_group: resource_group,
            debug_info: None,
        }
    }

    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    pub fn build(self) -> Arc<Mutex<R>> {
        Arc::new(Mutex::new(R::new()))
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::debug_info;

    use crate::{Backend, Renderer};

    struct DummyBackend;
    impl Backend for DummyBackend {
        fn new() -> Self
        where
            Self: Sized,
        {
            Self
        }
    }

    #[test]
    fn new_resource_group() {
        let renderer = Renderer::<DummyBackend>::builder().build().unwrap();
        let resource_container = renderer
            .create_resource_container()
            .with_debug_info(debug_info!("my_resource_group"))
            .build();
        let texture = resource_container
            .texture2ds
            .create()
            .with_debug_info(debug_info!("my_texture"))
            .build();
        assert_eq!(texture.lock().width(), 0);
    }
}