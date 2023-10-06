use jeriya_shared::{debug_info, nalgebra::Matrix4, Handle};

use crate::{
    elements::rigid_mesh_group::RigidMeshGroup,
    instances::{
        instance_group::InstanceGroup,
        rigid_mesh_instance::{self, RigidMeshInstance},
    },
    transactions::PushEvent,
};

use super::rigid_mesh_collection::RigidMeshCollection;

pub struct RigidMeshInstanceCollection {
    rigid_mesh_instances: Vec<Handle<RigidMeshInstance>>,
}

impl RigidMeshInstanceCollection {
    /// Creates a new [`RigidMeshInstanceCollection`] from a [`RigidMeshCollection`].
    pub fn from_rigid_mesh_collection(
        rigid_mesh_collection: &RigidMeshCollection,
        rigid_mesh_group: &RigidMeshGroup,
        instance_group: &mut InstanceGroup,
        transaction: &mut impl PushEvent,
        transform: &Matrix4<f32>,
    ) -> rigid_mesh_instance::Result<Self> {
        let rigid_mesh_instances = rigid_mesh_collection
            .rigid_meshes()
            .iter()
            .enumerate()
            .map(|(rigid_mesh_index, rigid_mesh)| {
                let rigid_mesh = rigid_mesh_group.get(rigid_mesh).expect("RigidMesh not found");
                let rigid_mesh_instance_builder = RigidMeshInstance::builder()
                    .with_debug_info(debug_info!(format!(
                        "RigidMeshInstance-from-RigidMeshCollection-{}",
                        rigid_mesh_index
                    )))
                    .with_rigid_mesh(rigid_mesh)
                    .with_transform(transform.clone());
                instance_group
                    .rigid_mesh_instances()
                    .mutate_via(transaction)
                    .insert_with(rigid_mesh_instance_builder)
            })
            .collect::<rigid_mesh_instance::Result<Vec<_>>>()?;

        Ok(Self { rigid_mesh_instances })
    }

    /// Returns the [`RigidMeshInstance`]es.
    pub fn rigid_mesh_instances(&self) -> &[Handle<RigidMeshInstance>] {
        &self.rigid_mesh_instances
    }
}
