use std::sync::Arc;

use jeriya_shared::{debug_info, nalgebra::Vector3, thiserror, DebugInfo, Handle};

use crate::gpu_index_allocator::GpuIndexAllocation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttributeType {
    PointPositions,
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Mandatory attribute {0:?} missing")]
    MandatoryAttributeMissing(AttributeType),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct PointCloudAttributes {
    point_positions: Vec<Vector3<f32>>,
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

#[derive(Default)]
pub struct PointCloudAttributesBuilder {
    point_positions: Option<Vec<Vector3<f32>>>,
    debug_info: Option<DebugInfo>,
}

impl PointCloudAttributesBuilder {
    /// Sets the point positions of the [`PointCloudAttributes`]
    pub fn with_point_positions(mut self, point_positions: Vec<Vector3<f32>>) -> Self {
        self.point_positions = Some(point_positions);
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
        Ok(PointCloudAttributes {
            point_positions,
            handle,
            gpu_index_allocation,
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous-PointCloudAttributes")),
        })
    }
}
