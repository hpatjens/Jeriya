use std::sync::Arc;

use jeriya_shared::{debug_info, DebugInfo};

use crate::{
    gpu_index_allocator::IntoAllocateGpuIndex, rigid_mesh_instance::RigidMeshInstance, rigid_mesh_instance_group::RigidMeshInstanceGroup,
};

pub struct InstanceGroup {
    debug_info: DebugInfo,
    rigid_mesh_instance_group: RigidMeshInstanceGroup,
}

impl InstanceGroup {
    /// Creates a new [`InstanceGroup`]
    pub fn new(rigid_mesh_instance_allocate_gpu_index: &Arc<impl IntoAllocateGpuIndex<RigidMeshInstance>>, debug_info: DebugInfo) -> Self {
        let rigid_mesh_instance_group = RigidMeshInstanceGroup::new(
            rigid_mesh_instance_allocate_gpu_index,
            debug_info!(format!("{}-rigid-mesh-instance-group", debug_info.name())),
        );
        Self {
            rigid_mesh_instance_group,
            debug_info,
        }
    }

    /// Returns the [`RigidMeshInstanceGroup`] that manages the rigid mesh instances.
    pub fn rigid_mesh_instances(&mut self) -> &mut RigidMeshInstanceGroup {
        &mut self.rigid_mesh_instance_group
    }

    /// Returns the [`DebugInfo`] of the [`InstanceGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}
