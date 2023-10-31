use std::sync::Arc;

use jeriya_shared::{debug_info, thiserror, DebugInfo, Handle};

use crate::{gpu_index_allocator::GpuIndexAllocation, resources::point_cloud_attributes::PointCloudAttributes};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The PointCloudAttributes of the PointCloud are not set")]
    PointCloudAttributesNotSet,
    #[error("The allocation of the PointCloud failed")]
    AllocationFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct PointCloud {
    debug_info: DebugInfo,
    point_cloud_attributes: Arc<PointCloudAttributes>,
    handle: Handle<PointCloud>,
    gpu_index_allocation: GpuIndexAllocation<PointCloud>,
}

impl PointCloud {
    /// Creates a new [`PointCloudBuilder`] for a [`PointCloud`]
    pub fn builder() -> PointCloudBuilder {
        PointCloudBuilder::default()
    }

    /// Returns the [`PointCloudAttributes`] of the [`PointCloud`]
    pub fn point_cloud_attributes(&self) -> &Arc<PointCloudAttributes> {
        &self.point_cloud_attributes
    }

    /// Returns the [`Handle`] of the [`PointCloud`].
    pub fn handle(&self) -> &Handle<PointCloud> {
        &self.handle
    }

    /// Returns the [`GpuIndexAllocation`] of the [`PointCloud`]
    pub fn gpu_index_allocation(&self) -> &GpuIndexAllocation<PointCloud> {
        &self.gpu_index_allocation
    }

    /// Returns the [`DebugInfo`] of the [`PointCloud`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[derive(Default)]
pub struct PointCloudBuilder {
    debug_info: Option<DebugInfo>,
    point_cloud_attributes: Option<Arc<PointCloudAttributes>>,
}

impl PointCloudBuilder {
    /// Sets the [`PointCloudAttributes`] of the [`PointCloud`]
    pub fn with_point_cloud_attributes(mut self, point_cloud_attributes: Arc<PointCloudAttributes>) -> Self {
        self.point_cloud_attributes = Some(point_cloud_attributes);
        self
    }

    /// Sets the [`DebugInfo`] of the [`PointCloud`]
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Builds the [`PointCloud`]
    pub(crate) fn build(self, handle: Handle<PointCloud>, gpu_index_allocation: GpuIndexAllocation<PointCloud>) -> Result<PointCloud> {
        let point_cloud_attributes = self.point_cloud_attributes.ok_or(Error::PointCloudAttributesNotSet)?;
        Ok(PointCloud {
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous PointCloud")),
            point_cloud_attributes,
            handle,
            gpu_index_allocation,
        })
    }
}
