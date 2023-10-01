use jeriya_shared::DebugInfo;

pub struct RigidMeshGroup {
    debug_info: DebugInfo,
}

impl RigidMeshGroup {
    pub fn new(debug_info: DebugInfo) -> Self {
        Self { debug_info }
    }
}
