use std::sync::Arc;

use jeriya_content::asset_importer::AssetImporter;

/// Responsible for creating vulkan resources asynchronously and handing them over to the [`VulkanResourceCoordinator`].
pub struct VulkanResourcePreparer {
    // TODO: spawn thread
}

impl VulkanResourcePreparer {
    pub fn new(asset_importer: &Arc<AssetImporter>) -> Self {
        VulkanResourcePreparer {}
    }
}
