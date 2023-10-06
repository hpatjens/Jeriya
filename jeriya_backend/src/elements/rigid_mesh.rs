use std::sync::Arc;

use jeriya_shared::{debug_info, thiserror, DebugInfo, Handle};

use crate::{gpu_index_allocator::GpuIndexAllocation, resources::mesh_attributes::MeshAttributes};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The MeshAttributes of the RigidMesh are not set")]
    MeshAttributesNotSet,
    #[error("The allocation of the RigidMesh failed")]
    AllocationFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub enum Event {
    Noop,
    Insert(RigidMesh),
}

#[derive(Debug, Clone)]
pub struct RigidMesh {
    debug_info: DebugInfo,
    mesh_attributes: Arc<MeshAttributes>,
    handle: Handle<RigidMesh>,
    gpu_index_allocation: GpuIndexAllocation<RigidMesh>,
}

impl RigidMesh {
    /// Creates a new [`RigidMeshBuilder`] for a [`RigidMesh`]
    pub fn builder() -> RigidMeshBuilder {
        RigidMeshBuilder::new()
    }

    /// Returns the [`MeshAttributes`] of the [`RigidMesh`]
    pub fn mesh_attributes(&self) -> &Arc<MeshAttributes> {
        &self.mesh_attributes
    }

    /// Returns the [`Handle`] of the [`RigidMesh`].
    pub fn handle(&self) -> &Handle<RigidMesh> {
        &self.handle
    }

    /// Returns the [`GpuIndexAllocation`] of the [`RigidMesh`]
    pub fn gpu_index_allocation(&self) -> &GpuIndexAllocation<RigidMesh> {
        &self.gpu_index_allocation
    }

    /// Returns the [`DebugInfo`] of the [`RigidMesh`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

pub struct RigidMeshBuilder {
    debug_info: Option<DebugInfo>,
    mesh_attributes: Option<Arc<MeshAttributes>>,
}

impl RigidMeshBuilder {
    fn new() -> Self {
        Self {
            debug_info: None,
            mesh_attributes: None,
        }
    }

    /// Sets the [`MeshAttributes`] of the [`RigidMesh`]
    pub fn with_mesh_attributes(mut self, mesh_attributes: Arc<MeshAttributes>) -> Self {
        self.mesh_attributes = Some(mesh_attributes);
        self
    }

    /// Sets the [`DebugInfo`] of the [`RigidMesh`]
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Creates the [`RigidMesh`]
    pub(crate) fn build(self, handle: Handle<RigidMesh>, gpu_index_allocation: GpuIndexAllocation<RigidMesh>) -> Result<RigidMesh> {
        let mesh_attributes = self.mesh_attributes.ok_or(Error::MeshAttributesNotSet)?;
        Ok(RigidMesh {
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous RigidMesh")),
            mesh_attributes,
            handle,
            gpu_index_allocation,
        })
    }
}

#[cfg(test)]
mod tests {
    use jeriya_shared::nalgebra::Vector3;
    use jeriya_test::spectral::assert_that;

    use crate::{
        elements::{self, rigid_mesh_group::RigidMeshGroup},
        resources::{self, mesh_attributes_group::MeshAttributesGroup},
        transactions::Transaction,
    };

    use super::*;

    #[test]
    fn smoke() {
        let renderer_mock = resources::MockRenderer::new();
        let mut mesh_attributes_group = MeshAttributesGroup::new(&renderer_mock, debug_info!("my_mesh_attributes_group"));
        let mesh_attributes_builder = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)])
            .with_indices(vec![0])
            .with_debug_info(debug_info!("my_attributes"));
        let mesh_attributes = mesh_attributes_group.insert_with(mesh_attributes_builder).unwrap();

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
        transaction.process();

        let rigid_mesh = rigid_mesh_group.get(&rigid_mesh_handle).unwrap();
        assert_that!(rigid_mesh.debug_info().name()).is_equal_to("my_rigid_mesh");
        assert_that!(rigid_mesh.mesh_attributes()).is_equal_to(&mesh_attributes);
        assert_that!(rigid_mesh.handle()).is_equal_to(&Handle::zero());
        assert_that!(rigid_mesh.gpu_index_allocation()).is_equal_to(&GpuIndexAllocation::new_unchecked(0));
    }
}
