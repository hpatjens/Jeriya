use std::sync::{Arc, Weak};

use jeriya_shared::{DebugInfo, Handle, IndexingContainer};

use crate::{
    gpu_index_allocator::{AllocateGpuIndex, IntoAllocateGpuIndex},
    rigid_mesh_instance::{self, Error, RigidMeshInstance, RigidMeshInstanceBuilder},
    transactions::{self, PushEvent},
};

pub struct RigidMeshInstanceGroup {
    gpu_index_allocator: Weak<dyn AllocateGpuIndex<RigidMeshInstance>>,
    indexing_container: IndexingContainer<RigidMeshInstance>,
    debug_info: DebugInfo,
}

impl RigidMeshInstanceGroup {
    /// Creates a new [`RigidMeshInstanceGroup`]
    pub fn new(gpu_index_allocator: &Arc<impl IntoAllocateGpuIndex<RigidMeshInstance>>, debug_info: DebugInfo) -> Self {
        Self {
            gpu_index_allocator: gpu_index_allocator.into_gpu_index_allocator(),
            indexing_container: IndexingContainer::new(),
            debug_info,
        }
    }

    /// Returns the [`RigidMeshInstance`] with the given [`Handle`]
    pub fn get(&self, handle: &Handle<RigidMeshInstance>) -> Option<&RigidMeshInstance> {
        self.indexing_container.get(handle)
    }

    /// Returns the [`DebugInfo`] of the [`RigidMeshInstanceGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }

    /// Returns a [`RigidMeshInstanceGroupAccessMut`] that can be used to mutate the [`RigidMeshInstanceGroup`] via the given [`Transaction`] or [`TransactionRecorder`].
    pub fn mutate_via<'g, 't, P: PushEvent>(&'g mut self, transaction: &'t mut P) -> RigidMeshInstanceGroupAccessMut<'g, 't, P> {
        RigidMeshInstanceGroupAccessMut {
            rigid_mesh_group: self,
            transaction,
        }
    }
}

pub struct RigidMeshInstanceGroupAccessMut<'g, 't, P: PushEvent> {
    rigid_mesh_group: &'g mut RigidMeshInstanceGroup,
    transaction: &'t mut P,
}

impl<'g, 't, P: PushEvent> RigidMeshInstanceGroupAccessMut<'g, 't, P> {
    /// Inserts a [`RigidMeshInstance`] into the [`RigidMeshInstanceGroup`].
    pub fn insert_with(
        &mut self,
        rigid_mesh_instance_builder: RigidMeshInstanceBuilder,
    ) -> rigid_mesh_instance::Result<Handle<RigidMeshInstance>> {
        self.rigid_mesh_group
            .indexing_container
            .insert_with(|handle| {
                let gpu_index_allocator = self
                    .rigid_mesh_group
                    .gpu_index_allocator
                    .upgrade()
                    .expect("the gpu_index_allocator was dropped");
                let gpu_index_allocation = gpu_index_allocator.allocate_gpu_index().ok_or(Error::AllocationFailed)?;
                let result = rigid_mesh_instance_builder.build(handle.clone(), gpu_index_allocation);
                if let Err(_) = &result {
                    gpu_index_allocator.free_gpu_index(gpu_index_allocation);
                }
                result
            })
            .and_then(|handle| {
                let rigid_mesh = self
                    .rigid_mesh_group
                    .indexing_container
                    .get(&handle)
                    .expect("just inserted value not found")
                    .clone();
                self.transaction
                    .push_event(transactions::Event::RigidMeshInstance(rigid_mesh_instance::Event::Insert(
                        rigid_mesh.clone(),
                    )));
                Ok(handle)
            })
    }
}
