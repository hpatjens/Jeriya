use std::sync::Arc;

use crate::AshBackend;
use jeriya_shared::{
    debug_info,
    parking_lot::{Mutex, MutexGuard},
    AsDebugInfo, Camera, DebugInfo, Handle, IndexingContainer, ObjectContainerHandler, ObjectGroupGuard, ObjectGroupGuardHandler,
};

pub struct AshObjectContainer {
    debug_info: DebugInfo,
    cameras: Arc<Mutex<AshObjectGroup<Camera>>>,
}

pub struct AshObjectContainerHandler {
    object_container: Arc<AshObjectContainer>,
}

impl ObjectContainerHandler for AshObjectContainerHandler {
    type Backend = AshBackend;

    fn new(backend: &Self::Backend, debug_info: DebugInfo) -> jeriya_shared::Result<Self>
    where
        Self: Sized,
    {
        let object_container = Arc::new(AshObjectContainer {
            debug_info,
            cameras: Arc::new(Mutex::new(AshObjectGroup::new(debug_info!("cameras-ObjectGroup")))),
        });
        backend.object_containers.lock().push(object_container.clone());
        Ok(Self { object_container })
    }

    fn cameras(&self) -> jeriya_shared::ObjectGroupGuard<Camera, Self::Backend> {
        ObjectGroupGuard::new(AshObjectGroupGuardHandler::new(self.object_container.cameras.lock()))
    }
}

impl AsDebugInfo for AshObjectContainerHandler {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.object_container.debug_info
    }
}

pub struct AshObjectGroupGuardHandler<'a, T>
where
    T: 'a,
{
    mutex_guard: MutexGuard<'a, AshObjectGroup<T>>,
}

impl<'a, T> AshObjectGroupGuardHandler<'a, T> {
    fn new(mutex_guard: MutexGuard<'a, AshObjectGroup<T>>) -> Self
    where
        Self: Sized,
    {
        Self { mutex_guard }
    }
}

impl<T> ObjectGroupGuardHandler<T> for AshObjectGroupGuardHandler<'_, T> {
    type Backend = AshBackend;

    fn insert(&mut self, object: T) -> Handle<T> {
        self.mutex_guard.indexing_container.insert(object)
    }

    fn remove(&mut self, handle: &Handle<T>) -> Option<T>
    where
        T: Default,
    {
        self.mutex_guard.indexing_container.remove(handle)
    }

    fn get(&self, handle: &Handle<T>) -> Option<&T> {
        self.mutex_guard.indexing_container.get(handle)
    }

    fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        self.mutex_guard.indexing_container.get_mut(handle)
    }
}

impl<'a, T> AsDebugInfo for AshObjectGroupGuardHandler<'a, T> {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.mutex_guard.debug_info
    }
}

pub struct AshObjectGroup<T> {
    debug_info: DebugInfo,
    indexing_container: IndexingContainer<T>,
}

impl<T> AshObjectGroup<T> {
    fn new(debug_info: DebugInfo) -> Self {
        Self {
            debug_info,
            indexing_container: IndexingContainer::new(),
        }
    }
}
