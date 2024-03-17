use std::sync::Arc;

use jeriya_shared::{debug_info, thiserror, Handle};

use crate::{
    elements::{
        element_group::ElementGroup,
        rigid_mesh::{self, MeshRepresentation, RigidMesh},
    },
    resources::{
        mesh_attributes::{self, MeshAttributes},
        resource_group::ResourceGroup,
    },
    transactions::PushEvent,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("MeshAttributesError: {0}")]
    MeshAttributes(#[from] mesh_attributes::Error),
    #[error("RigidMeshError: {0}")]
    RigidMesh(#[from] rigid_mesh::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A collection of [`RigidMesh`]es with their [`MeshAttributes`].
pub struct RigidMeshCollection {
    mesh_attributes: Vec<Arc<MeshAttributes>>,
    rigid_meshes: Vec<Handle<RigidMesh>>,
}

impl RigidMeshCollection {
    /// Creates a new [`RigidMeshCollection`] from a Model.
    pub fn from_model(
        model: &jeriya_content::model::ModelAsset,
        resource_group: &mut ResourceGroup,
        element_group: &mut ElementGroup,
        transaction: &mut impl PushEvent,
    ) -> Result<Self> {
        let (mesh_attributes, rigid_meshes) = model
            .meshes
            .iter()
            .enumerate()
            .map(|(mesh_index, mesh)| insert_attributes_and_mesh(&model.name, mesh_index, mesh, resource_group, element_group, transaction))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .unzip();

        Ok(Self {
            mesh_attributes,
            rigid_meshes,
        })
    }

    /// Returns the [`MeshAttributes`]es.
    pub fn mesh_attributes(&self) -> &[Arc<MeshAttributes>] {
        &self.mesh_attributes
    }

    /// Returns the [`RigidMesh`]es.
    pub fn rigid_meshes(&self) -> &[Handle<RigidMesh>] {
        &self.rigid_meshes
    }
}

/// Inserts the [`MeshAttributes`] and [`RigidMesh`] into the [`ResourceGroup`] and [`ElementGroup`].
fn insert_attributes_and_mesh(
    model_name: &str,
    mesh_index: usize,
    mesh: &jeriya_content::model::Mesh,
    resource_group: &mut ResourceGroup,
    element_group: &mut ElementGroup,
    transaction: &mut impl PushEvent,
) -> Result<(Arc<MeshAttributes>, Handle<RigidMesh>)> {
    // Insert the MeshAttributes
    let mesh_attributes_builder = MeshAttributes::builder()
        .with_debug_info(debug_info!(format!("MeshAttributes-Model-{}-Mesh-{}", model_name, mesh_index)))
        .with_vertex_positions(mesh.simple_mesh.vertex_positions.clone())
        .with_vertex_normals(mesh.simple_mesh.vertex_normals.clone())
        .with_indices(mesh.simple_mesh.indices.clone())
        .with_meshlets(mesh.meshlets.clone());
    let mesh_attributes = resource_group.mesh_attributes().insert_with(mesh_attributes_builder)?;

    // Insert the RigidMesh
    let rigid_mesh_builder = RigidMesh::builder()
        .with_preferred_mesh_representation(MeshRepresentation::Simple)
        .with_debug_info(debug_info!(format!("RigidMesh-Model-{}-Mesh-{}", model_name, mesh_index)))
        .with_mesh_attributes(mesh_attributes.clone());
    let handle = element_group
        .rigid_meshes()
        .mutate_via(transaction)
        .insert_with(rigid_mesh_builder)?;

    Ok((mesh_attributes, handle))
}
