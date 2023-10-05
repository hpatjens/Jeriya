use jeriya_shared::{debug_info, thiserror, DebugInfo, Handle};

use crate::{elements::rigid_mesh::RigidMesh, gpu_index_allocator::GpuIndexAllocation};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The RigidMesh of the RigidMeshInstance is not set")]
    RigidMeshNotSet,
    #[error("The allocation of the RigidMeshInstance failed")]
    AllocationFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum Event {
    Noop,
    Insert(RigidMeshInstance),
}

#[derive(Debug)]
pub struct RigidMeshInstance {
    debug_info: DebugInfo,
    rigid_mesh: Handle<RigidMesh>,
    handle: Handle<RigidMeshInstance>,
    gpu_index_allocation: GpuIndexAllocation<RigidMeshInstance>,
}

impl RigidMeshInstance {
    pub fn builder() -> RigidMeshInstanceBuilder {
        RigidMeshInstanceBuilder::new()
    }

    /// Returns the [`Handle`] of the [`RigidMesh`] that this [`RigidMeshInstance`] is an instance of.
    pub fn rigid_mesh(&self) -> &Handle<RigidMesh> {
        &self.rigid_mesh
    }

    /// Returns the [`DebugInfo`] of the [`RigidMeshInstance`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

pub struct RigidMeshInstanceBuilder {
    debug_info: Option<DebugInfo>,
    rigid_mesh: Option<Handle<RigidMesh>>,
}

impl RigidMeshInstanceBuilder {
    fn new() -> Self {
        Self {
            debug_info: None,
            rigid_mesh: None,
        }
    }

    /// Sets the [`Handle`] of the [`RigidMesh`] that this [`RigidMeshInstance`] is an instance of.
    pub fn with_rigid_mesh(mut self, rigid_mesh: Handle<RigidMesh>) -> Self {
        self.rigid_mesh = Some(rigid_mesh);
        self
    }

    /// Sets the [`DebugInfo`] of the [`RigidMeshInstance`]
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Builds the [`RigidMeshInstance`]
    pub fn build(
        self,
        handle: Handle<RigidMeshInstance>,
        gpu_index_allocation: GpuIndexAllocation<RigidMeshInstance>,
    ) -> Result<RigidMeshInstance> {
        let rigid_mesh = self.rigid_mesh.ok_or(Error::RigidMeshNotSet)?;
        Ok(RigidMeshInstance {
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous RigidMeshInstance")),
            rigid_mesh,
            handle,
            gpu_index_allocation,
        })
    }
}
