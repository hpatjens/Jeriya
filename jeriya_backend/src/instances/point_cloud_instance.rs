use jeriya_shared::{debug_info, nalgebra::Matrix4, thiserror, DebugInfo, Handle};

use crate::{elements::point_cloud::PointCloud, gpu_index_allocator::GpuIndexAllocation};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The PointCloud of the PointCloudInstance is not set")]
    PointCloudNotSet,
    #[error("The allocation of the PointCloudInstance failed")]
    AllocationFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Event {
    Noop,
    Insert(PointCloudInstance),
}

#[derive(Debug, Clone)]
pub struct PointCloudInstance {
    point_cloud_handle: Handle<PointCloud>,
    point_cloud_gpu_index_allocation: GpuIndexAllocation<PointCloud>,
    handle: Handle<PointCloudInstance>,
    gpu_index_allocation: GpuIndexAllocation<PointCloudInstance>,
    transform: Matrix4<f32>,
    debug_info: DebugInfo,
}

impl PointCloudInstance {
    pub fn builder() -> PointCloudInstanceBuilder {
        PointCloudInstanceBuilder::default()
    }

    /// Returns the [`Handle`] of the [`PointCloud`] that this [`PointCloudInstance`] is an instance of.
    pub fn point_cloud_handle(&self) -> &Handle<PointCloud> {
        &self.point_cloud_handle
    }

    /// Returns the [`GpuIndexAllocation`] of the [`PointCloud`] that this [`PointCloudInstance`] is an instance of.
    pub fn point_cloud_gpu_index_allocation(&self) -> &GpuIndexAllocation<PointCloud> {
        &self.point_cloud_gpu_index_allocation
    }

    /// Returns the [`Handle`] of the [`PointCloudInstance`]
    pub fn handle(&self) -> &Handle<PointCloudInstance> {
        &self.handle
    }

    /// Returns the [`GpuIndexAllocation`] of the [`PointCloudInstance`]
    pub fn gpu_index_allocation(&self) -> &GpuIndexAllocation<PointCloudInstance> {
        &self.gpu_index_allocation
    }

    /// Returns the transform of the [`PointCloudInstance`]
    pub fn transform(&self) -> &Matrix4<f32> {
        &self.transform
    }

    /// Returns the [`DebugInfo`] of the [`PointCloudInstance`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[derive(Default)]
pub struct PointCloudInstanceBuilder {
    point_cloud_handle: Option<Handle<PointCloud>>,
    point_cloud_gpu_index_allocation: Option<GpuIndexAllocation<PointCloud>>,
    transform: Option<Matrix4<f32>>,
    debug_info: Option<DebugInfo>,
}

impl PointCloudInstanceBuilder {
    /// Sets the [`PointCloud`] of the [`PointCloudInstance`]
    pub fn with_point_cloud(mut self, point_cloud: &PointCloud) -> Self {
        self.point_cloud_handle = Some(point_cloud.handle().clone());
        self.point_cloud_gpu_index_allocation = Some(point_cloud.gpu_index_allocation().clone());
        self
    }

    /// Sets the transform of the [`PointCloudInstance`]
    pub fn with_transform(mut self, transform: Matrix4<f32>) -> Self {
        self.transform = Some(transform);
        self
    }

    /// Sets the [`DebugInfo`] of the [`PointCloudInstance`]
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Builds the [`PointCloudInstance`]
    pub(crate) fn build(
        self,
        handle: Handle<PointCloudInstance>,
        gpu_index_allocation: GpuIndexAllocation<PointCloudInstance>,
    ) -> Result<PointCloudInstance> {
        let point_cloud_handle = self.point_cloud_handle.ok_or(Error::PointCloudNotSet)?;
        let point_cloud_gpu_index_allocation = self.point_cloud_gpu_index_allocation.ok_or(Error::AllocationFailed)?;
        Ok(PointCloudInstance {
            point_cloud_handle,
            point_cloud_gpu_index_allocation,
            handle,
            gpu_index_allocation,
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous PointCloudInstance")),
            transform: self.transform.unwrap_or(Matrix4::identity()),
        })
    }
}
