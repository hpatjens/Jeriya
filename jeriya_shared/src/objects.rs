use std::sync::{Arc, Mutex};

use crate::{DebugInfo, Handle, IndexingContainer};

use crate::Camera;

#[derive(Default)]
pub struct ObjectContainer {
    pub debug_info: Option<DebugInfo>,
    pub cameras: Arc<Mutex<ObjectGroup<Camera>>>,
}

#[derive(Default)]
pub struct ObjectGroup<T> {
    indexing_container: IndexingContainer<T>,
}

impl<T> ObjectGroup<T> {
    /// Inserts a new object and returns a [`Handle`] to it.
    pub fn insert(&mut self, object: T) -> Handle<T> {
        self.indexing_container.insert(object)
    }
}

impl<T: Default> ObjectGroup<T> {
    /// Removes the object with the given handle and returns it.
    pub fn remove(&mut self, handle: &Handle<T>) -> Option<T> {
        self.indexing_container.remove(handle)
    }
}

pub struct ObjectContainerBuilder {
    debug_info: Option<DebugInfo>,
}

impl ObjectContainerBuilder {
    pub fn new() -> Self {
        Self { debug_info: None }
    }

    /// Sets a [`DebugInfo`] for the [`ObjectContainer`]
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    pub fn build(self) -> ObjectContainer {
        ObjectContainer {
            debug_info: self.debug_info,
            ..Default::default()
        }
    }
}