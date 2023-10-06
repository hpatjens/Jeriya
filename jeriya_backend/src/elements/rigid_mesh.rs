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
