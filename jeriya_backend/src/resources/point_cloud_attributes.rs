use std::sync::Arc;

use jeriya_content::point_cloud::clustered_point_cloud::{ClusterIndex, Page};
use jeriya_shared::{debug_info, nalgebra::Vector3, thiserror, ByteColor3, DebugInfo, Handle};

use crate::gpu_index_allocator::GpuIndexAllocation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttributeType {
    PointPositions,
    PointColors,
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Mandatory attribute {0:?} missing")]
    MandatoryAttributeMissing(AttributeType),
    #[error("Allocation failed")]
    AllocationFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct PointCloudAttributes {
    point_positions: Vec<Vector3<f32>>,
    point_colors: Vec<ByteColor3>,
    root_cluster_index: ClusterIndex,
    pages: Vec<Page>,
    handle: Handle<Arc<PointCloudAttributes>>,
    gpu_index_allocation: GpuIndexAllocation<PointCloudAttributes>,
    debug_info: DebugInfo,
}

impl PointCloudAttributes {
    /// Creates a new [`PointCloudAttributesBuilder`]
    pub fn builder() -> PointCloudAttributesBuilder {
        PointCloudAttributesBuilder::default()
    }

    /// Returns the point positions of the `PointCloudAttributes`
    pub fn point_positions(&self) -> &[Vector3<f32>] {
        &self.point_positions
    }

    /// Returns the point colors of the `PointCloudAttributes`
    pub fn point_colors(&self) -> &[ByteColor3] {
        &self.point_colors
    }

    /// Returns the pages of the `PointCloudAttributes`
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Returns the root cluster index of the `PointCloudAttributes`
    pub fn root_cluster_index(&self) -> ClusterIndex {
        self.root_cluster_index.clone()
    }

    /// Returns the [`Handle`] of the `PointCloudAttributes`
    pub fn handle(&self) -> &Handle<Arc<PointCloudAttributes>> {
        &self.handle
    }

    /// Returns the [`GpuIndexAllocation`] of the `PointCloudAttributes`
    pub fn gpu_index_allocation(&self) -> &GpuIndexAllocation<PointCloudAttributes> {
        &self.gpu_index_allocation
    }

    /// Returns the [`DebugInfo`] of the `PointCloudAttributes`
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

/// Represents the state of the point cloud on the GPU
#[derive(Debug)]
pub enum PointCloudAttributesGpuState {
    /// The point cloud is currently being uploaded to the GPU
    WaitingForUpload { point_positions: Arc<Vec<Vector3<f32>>> },
    /// The point cloud has been uploaded to the GPU
    Uploaded,
}

#[derive(Default)]
pub struct PointCloudAttributesBuilder {
    point_positions: Option<Vec<Vector3<f32>>>,
    point_colors: Option<Vec<ByteColor3>>,
    pages: Option<Vec<Page>>,
    root_cluster_index: Option<ClusterIndex>,
    debug_info: Option<DebugInfo>,
}

impl PointCloudAttributesBuilder {
    /// Sets the point positions of the [`PointCloudAttributes`]
    pub fn with_point_positions(mut self, point_positions: Vec<Vector3<f32>>) -> Self {
        self.point_positions = Some(point_positions);
        self
    }

    /// Sets the point colors of the [`PointCloudAttributes`]
    pub fn with_point_colors(mut self, point_colors: Vec<ByteColor3>) -> Self {
        self.point_colors = Some(point_colors);
        self
    }

    /// Sets the pages of the [`PointCloudAttributes`]
    pub fn with_pages(mut self, pages: Vec<Page>) -> Self {
        self.pages = Some(pages);
        self
    }

    /// Sets the root cluster index of the [`PointCloudAttributes`]
    pub fn with_root_cluster_index(mut self, root_cluster_index: ClusterIndex) -> Self {
        self.root_cluster_index = Some(root_cluster_index);
        self
    }

    /// Sets the [`DebugInfo`] of the [`PointCloudAttributes`]
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Builds the [`PointCloudAttributes`]
    pub fn build(
        self,
        handle: Handle<Arc<PointCloudAttributes>>,
        gpu_index_allocation: GpuIndexAllocation<PointCloudAttributes>,
    ) -> Result<PointCloudAttributes> {
        let point_positions = self
            .point_positions
            .ok_or(Error::MandatoryAttributeMissing(AttributeType::PointPositions))?;
        let point_colors = self
            .point_colors
            .ok_or(Error::MandatoryAttributeMissing(AttributeType::PointColors))?;
        Ok(PointCloudAttributes {
            point_positions,
            point_colors,
            root_cluster_index: self.root_cluster_index.unwrap_or_default(),
            pages: self.pages.unwrap_or_default(),
            handle,
            gpu_index_allocation,
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous-PointCloudAttributes")),
        })
    }
}
