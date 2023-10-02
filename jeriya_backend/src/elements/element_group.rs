use std::sync::Arc;

use jeriya_shared::{debug_info, DebugInfo};

use crate::gpu_index_allocator::IntoAllocateGpuIndex;

use super::{rigid_mesh::RigidMesh, rigid_mesh_group::RigidMeshGroup};

pub struct ElementGroup {
    rigid_mesh_group: RigidMeshGroup,
    debug_info: DebugInfo,
}

impl ElementGroup {
    /// Creates a new [`ElementGroup`]
    pub fn new(rigid_mesh_allocate_gpu_index: &Arc<impl IntoAllocateGpuIndex<RigidMesh>>, debug_info: DebugInfo) -> Self {
        let rigid_mesh_group = RigidMeshGroup::new(
            rigid_mesh_allocate_gpu_index,
            debug_info!(format!("{}-rigid-mesh-group", debug_info.name())),
        );
        Self {
            rigid_mesh_group,
            debug_info,
        }
    }

    /// Returns the [`RigidMeshGroup`] that manages the rigid meshes.
    pub fn rigid_meshes(&mut self) -> &mut RigidMeshGroup {
        &mut self.rigid_mesh_group
    }

    /// Returns the [`DebugInfo`] of the [`ElementGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}
