use std::sync::{Arc, Weak};

use jeriya_shared::{DebugInfo, Handle, IndexingContainer};

use crate::{
    gpu_index_allocator::{AllocateGpuIndex, ProvideAllocateGpuIndex},
    transactions::{self, PushEvent},
};

use super::rigid_mesh::{self, Error, RigidMesh, RigidMeshBuilder};

pub struct RigidMeshGroup {
    gpu_index_allocator: Weak<dyn AllocateGpuIndex<RigidMesh>>,
    indexing_container: IndexingContainer<RigidMesh>,
    debug_info: DebugInfo,
}

impl RigidMeshGroup {
    /// Creates a new [`RigidMeshGroup`]
    pub(crate) fn new(gpu_index_allocator: &Arc<impl ProvideAllocateGpuIndex<RigidMesh>>, debug_info: DebugInfo) -> Self {
        Self {
            gpu_index_allocator: gpu_index_allocator.provide_gpu_index_allocator(),
            debug_info,
            indexing_container: IndexingContainer::new(),
        }
    }

    /// Returns the [`RigidMesh`] with the given [`Handle`]
    pub fn get(&self, handle: &Handle<RigidMesh>) -> Option<&RigidMesh> {
        self.indexing_container.get(handle)
    }

    /// Returns the [`DebugInfo`] of the [`RigidMeshGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }

    /// Returns a [`RigidMeshGroupAccessMut`] that can be used to mutate the [`RigidMeshGroup`] via the given [`Transaction`] or [`TransactionRecorder`].
    pub fn mutate_via<'g, 't, P: PushEvent>(&'g mut self, transaction: &'t mut P) -> RigidMeshGroupAccessMut<'g, 't, P> {
        RigidMeshGroupAccessMut {
            rigid_mesh_group: self,
            transaction,
        }
    }
}

pub struct RigidMeshGroupAccessMut<'g, 't, P: PushEvent> {
    rigid_mesh_group: &'g mut RigidMeshGroup,
    transaction: &'t mut P,
}

impl<'g, 't, P: PushEvent> RigidMeshGroupAccessMut<'g, 't, P> {
    /// Inserts a [`RigidMesh`] into the [`RigidMeshGroup`].
    pub fn insert_with(&mut self, rigid_mesh_builder: RigidMeshBuilder) -> rigid_mesh::Result<Handle<RigidMesh>> {
        self.rigid_mesh_group
            .indexing_container
            .insert_with(|handle| {
                let gpu_index_allocator = self
                    .rigid_mesh_group
                    .gpu_index_allocator
                    .upgrade()
                    .expect("the gpu_index_allocator was dropped");
                let gpu_index_allocation = gpu_index_allocator.allocate_gpu_index().ok_or(Error::AllocationFailed)?;
                let result = rigid_mesh_builder.build(handle.clone(), gpu_index_allocation);
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
                    .push_event(transactions::Event::RigidMesh(rigid_mesh::Event::Insert(rigid_mesh.clone())));
                Ok(handle)
            })
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::debug_info;
    use jeriya_test::spectral::assert_that;

    use crate::{
        elements, gpu_index_allocator::GpuIndexAllocation, resources::tests::new_dummy_mesh_attributes, transactions::Transaction,
    };

    use super::*;

    #[test]
    fn smoke() {
        let mesh_attributes = new_dummy_mesh_attributes();

        let renderer_mock = elements::MockRenderer::new();
        let mut transaction = Transaction::new();
        let mut rigid_mesh_group = RigidMeshGroup::new(&renderer_mock, debug_info!("my_rigid_mesh_group"));
        let rigid_mesh_builder = RigidMesh::builder()
            .with_mesh_attributes(mesh_attributes.clone())
            .with_debug_info(debug_info!("my_rigid_mesh"));
        let rigid_mesh_handle = rigid_mesh_group
            .mutate_via(&mut transaction)
            .insert_with(rigid_mesh_builder)
            .unwrap();

        let rigid_mesh = rigid_mesh_group.get(&rigid_mesh_handle).unwrap();
        assert_that!(rigid_mesh.debug_info().name()).is_equal_to("my_rigid_mesh");
        assert_that!(rigid_mesh.mesh_attributes()).is_equal_to(&mesh_attributes);
        assert_that!(rigid_mesh.handle()).is_equal_to(&Handle::zero());
        assert_that!(rigid_mesh.gpu_index_allocation()).is_equal_to(&GpuIndexAllocation::new_unchecked(0));

        // Assert Transaction
        assert_eq!(transaction.len(), 1);
        let first = transaction.process().into_iter().next().unwrap();
        assert!(matches!(first, transactions::Event::RigidMesh(rigid_mesh::Event::Insert(_))));

        // Assert GpuIndexAllocator
        assert_eq!(renderer_mock.backend.rigid_mesh_gpu_index_allocator.borrow_mut().len(), 1);
    }
}
