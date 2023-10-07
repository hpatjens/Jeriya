use jeriya_shared::{debug_info, nalgebra::Matrix4, thiserror, DebugInfo, Handle};

use crate::{elements::rigid_mesh::RigidMesh, gpu_index_allocator::GpuIndexAllocation};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The RigidMesh of the RigidMeshInstance is not set")]
    RigidMeshNotSet,
    #[error("The allocation of the RigidMeshInstance failed")]
    AllocationFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum Event {
    Noop,
    Insert(RigidMeshInstance),
}

#[derive(Debug, Clone)]
pub struct RigidMeshInstance {
    rigid_mesh_handle: Handle<RigidMesh>,
    rigid_mesh_gpu_index_allocation: GpuIndexAllocation<RigidMesh>,
    handle: Handle<RigidMeshInstance>,
    gpu_index_allocation: GpuIndexAllocation<RigidMeshInstance>,
    transform: Matrix4<f32>,
    debug_info: DebugInfo,
}

impl RigidMeshInstance {
    pub fn builder() -> RigidMeshInstanceBuilder {
        RigidMeshInstanceBuilder::new()
    }

    /// Returns the [`Handle`] of the [`RigidMesh`] that this [`RigidMeshInstance`] is an instance of.
    pub fn rigid_mesh_handle(&self) -> &Handle<RigidMesh> {
        &self.rigid_mesh_handle
    }

    /// Returns the [`GpuIndexAllocation`] of the [`RigidMesh`] that this [`RigidMeshInstance`] is an instance of.
    pub fn rigid_mesh_gpu_index_allocation(&self) -> &GpuIndexAllocation<RigidMesh> {
        &self.rigid_mesh_gpu_index_allocation
    }

    /// Returns the [`Handle`] of the [`RigidMeshInstance`]
    pub fn handle(&self) -> &Handle<RigidMeshInstance> {
        &self.handle
    }

    /// Returns the [`GpuIndexAllocation`] of the [`RigidMeshInstance`]
    pub fn gpu_index_allocation(&self) -> &GpuIndexAllocation<RigidMeshInstance> {
        &self.gpu_index_allocation
    }

    /// Returns the transform of the [`RigidMeshInstance`]
    pub fn transform(&self) -> &Matrix4<f32> {
        &self.transform
    }

    /// Returns the [`DebugInfo`] of the [`RigidMeshInstance`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

pub struct RigidMeshInstanceBuilder {
    rigid_mesh_handle: Option<Handle<RigidMesh>>,
    rigid_mesh_gpu_index_allocation: Option<GpuIndexAllocation<RigidMesh>>,
    transform: Option<Matrix4<f32>>,
    debug_info: Option<DebugInfo>,
}

impl RigidMeshInstanceBuilder {
    fn new() -> Self {
        Self {
            rigid_mesh_handle: None,
            rigid_mesh_gpu_index_allocation: None,
            transform: None,
            debug_info: None,
        }
    }

    /// Sets the [`Handle`] of the [`RigidMesh`] that this [`RigidMeshInstance`] is an instance of.
    pub fn with_rigid_mesh(mut self, rigid_mesh: &RigidMesh) -> Self {
        self.rigid_mesh_handle = Some(rigid_mesh.handle().clone());
        self.rigid_mesh_gpu_index_allocation = Some(rigid_mesh.gpu_index_allocation().clone());
        self
    }

    /// Sets the transform of the [`RigidMeshInstance`]
    pub fn with_transform(mut self, transform: Matrix4<f32>) -> Self {
        self.transform = Some(transform);
        self
    }

    /// Sets the [`DebugInfo`] of the [`RigidMeshInstance`]
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Builds the [`RigidMeshInstance`]
    pub(crate) fn build(
        self,
        handle: Handle<RigidMeshInstance>,
        gpu_index_allocation: GpuIndexAllocation<RigidMeshInstance>,
    ) -> Result<RigidMeshInstance> {
        let rigid_mesh_handle = self.rigid_mesh_handle.ok_or(Error::RigidMeshNotSet)?;
        let rigid_mesh_gpu_index_allocation = self.rigid_mesh_gpu_index_allocation.ok_or(Error::AllocationFailed)?;
        Ok(RigidMeshInstance {
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous RigidMeshInstance")),
            rigid_mesh_handle,
            rigid_mesh_gpu_index_allocation,
            handle,
            gpu_index_allocation,
            transform: self.transform.unwrap_or(Matrix4::identity()),
        })
    }
}
