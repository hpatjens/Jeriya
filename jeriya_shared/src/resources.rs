mod static_mesh;
mod texture2d;

pub use static_mesh::*;
pub use texture2d::*;

use std::sync::Arc;

use crate::{parking_lot::Mutex, DebugInfo};

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
    pub fn new() -> Self {
        Self { debug_info: None }
    }

    /// Sets a [`DebugInfo`] for the [`ResourceContainer`]
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
    data: Vec<Arc<Mutex<R>>>,
}

impl ResourceGroup<Texture2d> {
    pub fn create(&mut self) -> ResourceBuilder<Texture2d> {
        ResourceBuilder::new(self)
    }
}

/// Builder for a [`Resource`]
pub struct ResourceBuilder<'resgr, R> {
    resource_group: &'resgr mut ResourceGroup<R>,
    debug_info: Option<DebugInfo>,
}

impl<'resgr, R> ResourceBuilder<'resgr, R>
where
    R: Resource,
{
    fn new(resource_group: &'resgr mut ResourceGroup<R>) -> Self {
        Self {
            resource_group,
            debug_info: None,
        }
    }

    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    pub fn build(self) -> Arc<Mutex<R>> {
        let resource = Arc::new(Mutex::new(R::new()));
        self.resource_group.data.push(resource.clone());
        resource
    }
}
