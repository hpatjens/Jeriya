use jeriya_shared::DebugInfo;

pub struct InstanceGroup {
    debug_info: DebugInfo,
}

impl InstanceGroup {
    /// Creates a new [`InstanceGroup`]
    pub fn new(debug_info: DebugInfo) -> Self {
        Self { debug_info }
    }

    /// Returns the [`DebugInfo`] of the [`InstanceGroup`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}
