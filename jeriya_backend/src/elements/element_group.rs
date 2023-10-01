use jeriya_shared::{debug_info, DebugInfo};

use crate::transactions::PushEvent;

use super::rigid_mesh_group::RigidMeshGroup;

pub struct ElementGroup {
    rigid_mesh_group: RigidMeshGroup,
    debug_info: DebugInfo,
}

impl ElementGroup {
    /// Creates a new [`ElementGroup`]
    pub fn new(debug_info: DebugInfo) -> Self {
        let rigid_mesh_group = RigidMeshGroup::new(debug_info!(format!("{}-rigid-mesh-group", debug_info.name())));
        Self {
            rigid_mesh_group,
            debug_info,
        }
    }

    /// Returns the [`RigidMeshGroup`] that manages the rigid meshes.
    pub fn rigid_meshes(&mut self) -> &mut RigidMeshGroup {
        &mut self.rigid_mesh_group
    }
}
