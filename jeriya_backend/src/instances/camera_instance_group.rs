use std::sync::{Arc, Weak};

use jeriya_shared::{DebugInfo, Handle, IndexingContainer};

use crate::{
    gpu_index_allocator::{AllocateGpuIndex, IntoAllocateGpuIndex},
    transactions::{self, PushEvent},
};

use super::camera_instance::{self, CameraInstance, CameraInstanceBuilder, Error};

pub struct CameraInstanceGroup {
    gpu_index_allocator: Weak<dyn AllocateGpuIndex<CameraInstance>>,
    indexing_container: IndexingContainer<CameraInstance>,
    debug_info: DebugInfo,
}

impl CameraInstanceGroup {
    /// Creates a new [`CameraInstanceGroup`]
    pub fn new(gpu_index_allocator: &Arc<impl IntoAllocateGpuIndex<CameraInstance>>, debug_info: DebugInfo) -> Self {
        Self {
            gpu_index_allocator: gpu_index_allocator.into_gpu_index_allocator(),
            indexing_container: IndexingContainer::new(),
            debug_info,
        }
    }

    /// Returns the [`CameraInstance`] with the given [`Handle`]
    pub fn get(&self, handle: &Handle<CameraInstance>) -> Option<&CameraInstance> {
        self.indexing_container.get(handle)
    }

    /// Returns the [`CameraInstance`] with the given [`Handle`] mutably
    pub fn get_mut(&mut self, handle: &Handle<CameraInstance>) -> Option<&mut CameraInstance> {
        self.indexing_container.get_mut(handle)
    }

    /// Returns the [`DebugInfo`] of the [`CameraInstanceGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }

    /// Returns a [`CameraInstanceGroupAccessMut`] that can be used to mutate the [`CameraInstanceGroup`] via the given [`Transaction`] or [`TransactionRecorder`].
    pub fn mutate_via<'g, 't, P: PushEvent>(&'g mut self, transaction: &'t mut P) -> CameraInstanceGroupAccessMut<'g, 't, P> {
        CameraInstanceGroupAccessMut {
            camera_group: self,
            transaction,
        }
    }
}

pub struct CameraInstanceGroupAccessMut<'g, 't, P: PushEvent> {
    camera_group: &'g mut CameraInstanceGroup,
    transaction: &'t mut P,
}

impl<'g, 't, P: PushEvent> CameraInstanceGroupAccessMut<'g, 't, P> {
    /// Inserts a [`CameraInstance`] into the [`CameraInstanceGroup`].
    pub fn insert_with(&mut self, camera_instance_builder: CameraInstanceBuilder) -> camera_instance::Result<Handle<CameraInstance>> {
        self.camera_group
            .indexing_container
            .insert_with(|handle| {
                let gpu_index_allocator = self
                    .camera_group
                    .gpu_index_allocator
                    .upgrade()
                    .expect("the gpu_index_allocator was dropped");
                let gpu_index_allocation = gpu_index_allocator.allocate_gpu_index().ok_or(Error::AllocationFailed)?;
                let result = camera_instance_builder.build(handle.clone(), gpu_index_allocation);
                if let Err(_) = &result {
                    gpu_index_allocator.free_gpu_index(gpu_index_allocation);
                }
                result
            })
            .and_then(|handle| {
                let camera = self
                    .camera_group
                    .indexing_container
                    .get(&handle)
                    .expect("just inserted value not found")
                    .clone();
                self.transaction
                    .push_event(transactions::Event::CameraInstance(camera_instance::Event::Insert(camera.clone())));
                Ok(handle)
            })
    }
}
