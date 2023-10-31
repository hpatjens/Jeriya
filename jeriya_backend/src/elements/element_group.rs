use std::sync::Arc;

use jeriya_shared::{debug_info, DebugInfo};

use crate::gpu_index_allocator::ProvideAllocateGpuIndex;

use super::{
    camera::Camera, camera_group::CameraGroup, point_cloud::PointCloud, point_cloud_group::PointCloudGroup, rigid_mesh::RigidMesh,
    rigid_mesh_group::RigidMeshGroup,
};

pub struct ElementGroup {
    camera_group: CameraGroup,
    rigid_mesh_group: RigidMeshGroup,
    point_cloud_group: PointCloudGroup,
    debug_info: DebugInfo,
}

impl ElementGroup {
    /// Creates a new [`ElementGroup`]
    pub fn new<A>(allocate_gpu_index: &Arc<A>, debug_info: DebugInfo) -> Self
    where
        A: ProvideAllocateGpuIndex<RigidMesh> + ProvideAllocateGpuIndex<Camera> + ProvideAllocateGpuIndex<PointCloud>,
    {
        let camera_group = CameraGroup::new(allocate_gpu_index, debug_info!(format!("{}-camera-group", debug_info.name())));
        let rigid_mesh_group = RigidMeshGroup::new(allocate_gpu_index, debug_info!(format!("{}-rigid-mesh-group", debug_info.name())));
        let point_cloud_group = PointCloudGroup::new(allocate_gpu_index, debug_info!(format!("{}-point-cloud-group", debug_info.name())));
        Self {
            camera_group,
            rigid_mesh_group,
            point_cloud_group,
            debug_info,
        }
    }

    /// Returns the [`CameraGroup`] that manages the cameras.
    pub fn cameras(&mut self) -> &mut CameraGroup {
        &mut self.camera_group
    }

    /// Returns the [`RigidMeshGroup`] that manages the rigid meshes.
    pub fn rigid_meshes(&mut self) -> &mut RigidMeshGroup {
        &mut self.rigid_mesh_group
    }

    /// Returns the [`PointCloudGroup`] that manages the point clouds.
    pub fn point_clouds(&mut self) -> &mut PointCloudGroup {
        &mut self.point_cloud_group
    }

    /// Returns the [`DebugInfo`] of the [`ElementGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}
