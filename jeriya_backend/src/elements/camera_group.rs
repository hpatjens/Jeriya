use std::sync::{Arc, Weak};

use jeriya_shared::{DebugInfo, Handle, IndexingContainer};

use crate::{
    elements::camera::Camera,
    gpu_index_allocator::{AllocateGpuIndex, ProvideAllocateGpuIndex},
    transactions::{self, PushEvent},
};

use super::camera::{self, CameraBuilder, Error};

pub struct CameraGroup {
    gpu_index_allocator: Weak<dyn AllocateGpuIndex<Camera>>,
    indexing_container: IndexingContainer<Camera>,
    debug_info: DebugInfo,
}

impl CameraGroup {
    /// Creates a new [`CameraGroup`]
    pub(crate) fn new(gpu_index_allocator: &Arc<impl ProvideAllocateGpuIndex<Camera>>, debug_info: DebugInfo) -> Self {
        Self {
            gpu_index_allocator: gpu_index_allocator.provide_gpu_index_allocator(),
            debug_info,
            indexing_container: IndexingContainer::new(),
        }
    }

    /// Returns the [`Camera`] with the given [`Handle`]
    pub fn get(&self, handle: &Handle<Camera>) -> Option<&Camera> {
        self.indexing_container.get(handle)
    }

    /// Returns the [`Camera`] with the given [`Handle`] mutably
    pub fn get_mut(&mut self, handle: &Handle<Camera>) -> Option<&mut Camera> {
        self.indexing_container.get_mut(handle)
    }

    /// Returns the [`DebugInfo`] of the [`CameraGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }

    /// Returns a [`CameraGroupAccessMut`] that can be used to mutate the [`CameraGroup`] via the given [`Transaction`] or [`TransactionRecorder`].
    pub fn mutate_via<'g, 't, P: PushEvent>(&'g mut self, transaction: &'t mut P) -> CameraGroupAccessMut<'g, 't, P> {
        CameraGroupAccessMut {
            camera_group: self,
            transaction,
        }
    }
}

pub struct CameraGroupAccessMut<'g, 't, P: PushEvent> {
    camera_group: &'g mut CameraGroup,
    transaction: &'t mut P,
}

impl<'g, 't, P: PushEvent> CameraGroupAccessMut<'g, 't, P> {
    /// Inserts a [`Camera`] into the [`CameraGroup`].
    pub fn insert_with(&mut self, camera_builder: CameraBuilder) -> camera::Result<Handle<Camera>> {
        self.camera_group
            .indexing_container
            .insert_with(|handle| {
                let gpu_index_allocator = self
                    .camera_group
                    .gpu_index_allocator
                    .upgrade()
                    .expect("the gpu_index_allocator was dropped");
                let gpu_index_allocation = gpu_index_allocator.allocate_gpu_index().ok_or(Error::AllocationFailed)?;
                let result = camera_builder.build(*handle, gpu_index_allocation);
                if result.is_err() {
                    gpu_index_allocator.free_gpu_index(gpu_index_allocation);
                }
                result
            })
            .map(|handle| {
                let camera = self
                    .camera_group
                    .indexing_container
                    .get(&handle)
                    .expect("just inserted value not found")
                    .clone();
                self.transaction
                    .push_event(transactions::Event::Camera(camera::Event::Insert(camera.clone())));
                handle
            })
    }
}
