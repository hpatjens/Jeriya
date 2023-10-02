use std::sync::Arc;

use jeriya_shared::{debug_info, log::info, nalgebra::Vector3, thiserror, AsDebugInfo, DebugInfo, Handle};

use crate::gpu_index_allocator::GpuIndexAllocation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttributeType {
    Positions,
    Normals,
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("The {0:?} are missing")]
    MandatoryAttributeMissing(AttributeType),
    #[error("The index references a vertex that doesn't exist")]
    WrongIndex(usize),
    #[error("The number of attributes doesn't match the number of vertices")]
    WrongSize { expected: usize, got: usize },
    #[error("Allocation failed")]
    AllocationFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Vertex data for a mesh
#[derive(Debug, PartialEq)]
pub struct MeshAttributes {
    vertex_positions: Vec<Vector3<f32>>,
    vertex_normals: Vec<Vector3<f32>>,
    indices: Option<Vec<u32>>,
    handle: Handle<Arc<MeshAttributes>>,
    gpu_index_allocation: GpuIndexAllocation<MeshAttributes>,
    debug_info: DebugInfo,
}

impl MeshAttributes {
    /// Creates a new [`MeshAttributesBuilder`] for a mesh
    pub fn builder() -> MeshAttributeBuilder {
        MeshAttributeBuilder::new()
    }

    /// Returns the vertex positions
    pub fn vertex_positions(&self) -> &Vec<Vector3<f32>> {
        &self.vertex_positions
    }

    /// Returns the vertex normals
    pub fn vertex_normals(&self) -> &Vec<Vector3<f32>> {
        &self.vertex_normals
    }

    /// Returns the indices
    pub fn indices(&self) -> Option<&Vec<u32>> {
        self.indices.as_ref()
    }

    /// Returns the [`Handle`] of the [`MeshAttributes`].
    ///
    /// This can be used to query the [`MeshAttributes`] from the [`MeshAttributesGroup`] in which it is stored.
    pub fn handle(&self) -> &Handle<Arc<MeshAttributes>> {
        &self.handle
    }

    /// Returns the GPU index allocation
    pub fn gpu_index_allocation(&self) -> &GpuIndexAllocation<MeshAttributes> {
        &self.gpu_index_allocation
    }

    /// Returns the [`DebugInfo`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

impl AsDebugInfo for MeshAttributes {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

/// Represents the state of the mesh on the GPU
#[derive(Debug)]
pub enum MeshAttributesGpuState {
    /// The mesh is currently being uploaded to the GPU
    WaitingForUpload {
        vertex_positions: Arc<Vec<Vector3<f32>>>,
        vertex_normals: Arc<Vec<Vector3<f32>>>,
        indices: Option<Arc<Vec<u32>>>,
    },
    /// The mesh has been uploaded to the GPU
    Uploaded,
}

/// Builder for [`MeshAttributes`]
///
/// This is used to create [`MeshAttributes`] in the [`MeshAttributesGroup`]. Pass the [`MeshAttributeBuilder`]
/// to [`MeshAttributesGroup::insert_with`] method to create a [`MeshAttributes`].
pub struct MeshAttributeBuilder {
    vertex_positions: Option<Vec<Vector3<f32>>>,
    vertex_normals: Option<Vec<Vector3<f32>>>,
    indices: Option<Vec<u32>>,
    debug_info: Option<DebugInfo>,
}

impl MeshAttributeBuilder {
    fn new() -> Self {
        Self {
            vertex_positions: None,
            vertex_normals: None,
            indices: None,
            debug_info: None,
        }
    }

    /// Sets the vertex positions of the [`InanimateMesh`]
    ///
    /// This is a required field
    pub fn with_vertex_positions(mut self, vertex_positions: Vec<Vector3<f32>>) -> Self {
        self.vertex_positions = Some(vertex_positions);
        self
    }

    /// Sets the vertex normals of the [`InanimateMesh`]
    ///
    /// This is a required field
    pub fn with_vertex_normals(mut self, vertex_normals: Vec<Vector3<f32>>) -> Self {
        self.vertex_normals = Some(vertex_normals);
        self
    }

    /// Sets the indices of the [`InanimateMesh`]
    ///
    /// This is an optional field
    pub fn with_indices(mut self, indices: Vec<u32>) -> Self {
        self.indices = Some(indices);
        self
    }

    /// Sets the debug info of the [`InanimateMesh`]
    ///
    /// This is an optional field
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Builds the [`MeshAttributes`]
    pub(crate) fn build(
        self,
        handle: Handle<Arc<MeshAttributes>>,
        gpu_index_allocation: GpuIndexAllocation<MeshAttributes>,
    ) -> Result<MeshAttributes> {
        let vertex_positions = self
            .vertex_positions
            .ok_or(Error::MandatoryAttributeMissing(AttributeType::Positions))?;
        let vertex_normals = self
            .vertex_normals
            .ok_or(Error::MandatoryAttributeMissing(AttributeType::Normals))?;
        // The vertex positions determine the expected number of attributes
        if vertex_positions.len() != vertex_normals.len() {
            return Err(Error::WrongSize {
                expected: vertex_positions.len(),
                got: vertex_normals.len(),
            });
        }
        // The indices must references existing vertices
        info!("Checking every index in the mesh");
        if let Some(indices) = &self.indices {
            for index in indices {
                if *index as usize >= vertex_positions.len() {
                    return Err(Error::WrongIndex(*index as usize));
                }
            }
        }
        Ok(MeshAttributes {
            vertex_positions,
            vertex_normals,
            indices: self.indices,
            handle,
            gpu_index_allocation,
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous-MeshAttributes")),
        })
    }
}

#[cfg(test)]
mod tests {
    use jeriya_test::spectral::asserting;

    use super::*;

    #[test]
    fn success() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let mesh_attributes = MeshAttributes::builder()
            .with_vertex_positions(vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(2.0, 0.0, 0.0),
            ])
            .with_vertex_normals(vec![
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
            ])
            .with_indices(vec![0, 1, 2])
            .with_debug_info(debug_info!("my_mesh"))
            .build(Handle::zero(), gpu_index_allocation)
            .unwrap();
        asserting("vertex positions")
            .that(mesh_attributes.vertex_positions())
            .is_equal_to(vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(2.0, 0.0, 0.0),
            ]);
        asserting("vertex normals").that(mesh_attributes.vertex_normals()).is_equal_to(vec![
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ]);
        asserting("indices")
            .that(&mesh_attributes.indices())
            .is_equal_to(Some(&vec![0, 1, 2]));
        asserting("debug info")
            .that(&mesh_attributes.debug_info.name())
            .is_equal_to(&"my_mesh");
    }

    #[test]
    fn vertex_positions_missing() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let result = MeshAttributes::builder().build(Handle::zero(), gpu_index_allocation);
        asserting("missing vertex positions")
            .that(&result)
            .is_equal_to(Err(Error::MandatoryAttributeMissing(AttributeType::Positions)));
    }

    #[test]
    fn vertex_normals_missing() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let result = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .build(Handle::zero(), gpu_index_allocation);
        asserting("missing vertex normals")
            .that(&result)
            .is_equal_to(Err(Error::MandatoryAttributeMissing(AttributeType::Normals)));
    }

    #[test]
    fn wrong_size() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let result = MeshAttributes::builder()
            .with_vertex_positions(vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(2.0, 0.0, 0.0),
            ])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)])
            .build(Handle::zero(), gpu_index_allocation);
        asserting("wrong size")
            .that(&result)
            .is_equal_to(Err(Error::WrongSize { expected: 3, got: 1 }));
    }

    #[test]
    fn wrong_index() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let result = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)])
            .with_indices(vec![1])
            .build(Handle::zero(), gpu_index_allocation);
        asserting("wrong size").that(&result).is_equal_to(Err(Error::WrongIndex(1)));
    }
}
