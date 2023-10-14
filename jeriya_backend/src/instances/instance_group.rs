use std::sync::Arc;

use jeriya_shared::{debug_info, DebugInfo};

use crate::{
    gpu_index_allocator::ProvideAllocateGpuIndex,
    instances::{rigid_mesh_instance::RigidMeshInstance, rigid_mesh_instance_group::RigidMeshInstanceGroup},
};

use super::{camera_instance::CameraInstance, camera_instance_group::CameraInstanceGroup};

pub struct InstanceGroup {
    debug_info: DebugInfo,
    camera_instance_group: CameraInstanceGroup,
    rigid_mesh_instance_group: RigidMeshInstanceGroup,
}

impl InstanceGroup {
    /// Creates a new [`InstanceGroup`]
    pub fn new<A>(allocate_gpu_index: &Arc<A>, debug_info: DebugInfo) -> Self
    where
        A: ProvideAllocateGpuIndex<RigidMeshInstance> + ProvideAllocateGpuIndex<CameraInstance>,
    {
        let camera_instance_group = CameraInstanceGroup::new(
            allocate_gpu_index,
            debug_info!(format!("{}-camera-instance-group", debug_info.name())),
        );
        let rigid_mesh_instance_group = RigidMeshInstanceGroup::new(
            allocate_gpu_index,
            debug_info!(format!("{}-rigid-mesh-instance-group", debug_info.name())),
        );
        Self {
            camera_instance_group,
            rigid_mesh_instance_group,
            debug_info,
        }
    }

    /// Returns the [`CameraInstanceGroup`] that manages the camera instances.
    pub fn camera_instances(&mut self) -> &mut CameraInstanceGroup {
        &mut self.camera_instance_group
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
