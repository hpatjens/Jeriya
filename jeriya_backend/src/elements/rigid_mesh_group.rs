use jeriya_shared::{DebugInfo, Handle, IndexingContainer};

use crate::transactions::{self, PushEvent};

use super::rigid_mesh::{self, RigidMesh, RigidMeshBuilder};

pub struct RigidMeshGroup {
    indexing_container: IndexingContainer<RigidMesh>,
    debug_info: DebugInfo,
}

impl RigidMeshGroup {
    /// Creates a new [`RigidMeshGroup`]
    pub(crate) fn new(debug_info: DebugInfo) -> Self {
        Self {
            debug_info,
            indexing_container: IndexingContainer::new(),
        }
    }

    /// Inserts a [`RigidMesh`] into the [`RigidMeshGroup`] by using a [`RigidMeshBuilder`]
    pub fn insert_with(
        &mut self,
        transaction: &mut impl PushEvent,
        rigid_mesh_builder: RigidMeshBuilder,
    ) -> rigid_mesh::Result<Handle<RigidMesh>> {
        self.indexing_container
            .insert_with(|handle| rigid_mesh_builder.build(handle.clone()))
            .and_then(|handle| {
                let rigid_mesh = self.indexing_container.get(&handle).expect("just inserted value not found").clone();
                transaction.push_event(transactions::Event::RigidMesh(rigid_mesh::Event::Insert(rigid_mesh.clone())));
                Ok(handle)
            })
    }

    /// Returns the [`DebugInfo`] of the [`RigidMeshGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::debug_info;

    use crate::{tests::new_dummy_mesh_attributes, transactions::Transaction};

    use super::*;

    #[test]
    fn smoke() {
        let mesh_attributes = new_dummy_mesh_attributes();

        let mut rigid_mesh_group = RigidMeshGroup::new(debug_info!("my_rigid_mesh_group"));
        let mut transaction = Transaction::new();
        let rigid_mesh_builder = RigidMesh::builder()
            .with_mesh_attributes(mesh_attributes)
            .with_debug_info(debug_info!("my_rigid_mesh"));
        let rigid_mesh_handle = rigid_mesh_group.insert_with(&mut transaction, rigid_mesh_builder).unwrap();

        let rigid_mesh = rigid_mesh_group.indexing_container.get(&rigid_mesh_handle).unwrap();
        assert_eq!(rigid_mesh.debug_info().name(), "my_rigid_mesh");

        assert_eq!(transaction.len(), 1);
        let first = transaction.iter().next().unwrap();
        assert!(matches!(first, transactions::Event::RigidMesh(rigid_mesh::Event::Insert(_))));

        transaction.set_is_processed(true);
    }
}
