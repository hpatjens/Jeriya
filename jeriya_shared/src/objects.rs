use std::sync::{Arc, Mutex};

use crate::{AsDebugInfo, Backend, DebugInfo, Handle, IndexingContainer};

use crate::Camera;

#[derive(Default)]
pub struct ObjectContainer<B: Backend> {
    object_container_handler: B::ObjectContainerHandler,
    pub cameras: Arc<Mutex<ObjectGroup<Camera>>>,
}

impl<B: Backend> ObjectContainer<B> {
    pub fn new(object_container_handler: B::ObjectContainerHandler) -> Self {
        Self {
            object_container_handler,
            cameras: Arc::new(Mutex::new(ObjectGroup::default())),
        }
    }
}

impl<B: Backend> AsDebugInfo for ObjectContainer<B> {
    fn as_debug_info(&self) -> &DebugInfo {
        self.object_container_handler.as_debug_info()
    }
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
