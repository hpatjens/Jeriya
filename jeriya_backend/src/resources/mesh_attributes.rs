use std::sync::Arc;

use jeriya_content::model::Meshlet;
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
    WrongIndex { index_index: usize, index_value: usize },
    #[error("A global index in a meshlet references a vertex that doesn't exist")]
    WrongGlobalMeshletIndex {
        meshlet_index: usize,
        index_index: usize,
        index_value: usize,
    },
    #[error("A local index in a meshlet references a global index that doesn't exist")]
    WrongLocalMeshletIndex {
        meshlet_index: usize,
        triangle_index: usize,
        index_value: usize,
    },
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
    meshlets: Option<Vec<Meshlet>>,
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

    /// Returns the meshlets
    pub fn meshlets(&self) -> Option<&Vec<Meshlet>> {
        self.meshlets.as_ref()
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
        meshlets: Option<Arc<Vec<Meshlet>>>,
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
    meshlets: Option<Vec<Meshlet>>,
    debug_info: Option<DebugInfo>,
}

impl MeshAttributeBuilder {
    fn new() -> Self {
        Self {
            vertex_positions: None,
            vertex_normals: None,
            indices: None,
            meshlets: None,
            debug_info: None,
        }
    }

    /// Sets the vertex positions of the [`MeshAttributes`]
    ///
    /// This is a required field
    pub fn with_vertex_positions(mut self, vertex_positions: Vec<Vector3<f32>>) -> Self {
        self.vertex_positions = Some(vertex_positions);
        self
    }

    /// Sets the vertex normals of the [`MeshAttributes`]
    ///
    /// This is a required field
    pub fn with_vertex_normals(mut self, vertex_normals: Vec<Vector3<f32>>) -> Self {
        self.vertex_normals = Some(vertex_normals);
        self
    }

    /// Sets the indices of the [`MeshAttributes`]
    ///
    /// This is an optional field
    pub fn with_indices(mut self, indices: Vec<u32>) -> Self {
        self.indices = Some(indices);
        self
    }

    /// Sets the meshlets of the [`MeshAttributes`]
    pub fn with_meshlets(mut self, meshlets: Vec<Meshlet>) -> Self {
        self.meshlets = Some(meshlets);
        self
    }

    /// Sets the debug info of the [`MeshAttributes`]
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
            for (index_index, index_value) in indices.iter().enumerate() {
                if *index_value as usize >= vertex_positions.len() {
                    return Err(Error::WrongIndex {
                        index_index,
                        index_value: *index_value as usize,
                    });
                }
            }
        }

        // The meshlet indices must references existing vertices
        info!("Checking every meshlet index in the mesh");
        if let Some(meshlets) = &self.meshlets {
            for (meshlet_index, meshlet) in meshlets.iter().enumerate() {
                for (index_index, index_value) in meshlet.global_indices.iter().enumerate() {
                    if *index_value as usize >= vertex_positions.len() {
                        return Err(Error::WrongGlobalMeshletIndex {
                            meshlet_index,
                            index_index,
                            index_value: *index_value as usize,
                        });
                    }
                }
                for (triangle_index, triangle_value) in meshlet.local_indices.iter().enumerate() {
                    for i in 0..triangle_value.len() {
                        if triangle_value[i] as usize >= vertex_positions.len() {
                            return Err(Error::WrongLocalMeshletIndex {
                                meshlet_index,
                                triangle_index,
                                index_value: triangle_value[i] as usize,
                            });
                        }
                    }
                }
            }
        }

        Ok(MeshAttributes {
            vertex_positions,
            vertex_normals,
            indices: self.indices,
            meshlets: self.meshlets,
            handle,
            gpu_index_allocation,
            debug_info: self.debug_info.unwrap_or_else(|| debug_info!("Anonymous-MeshAttributes")),
        })
    }
}

#[cfg(test)]
mod tests {
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
            .with_meshlets(vec![Meshlet {
                global_indices: vec![0],
                local_indices: vec![[0, 0, 0]],
            }])
            .with_debug_info(debug_info!("my_mesh"))
            .build(Handle::zero(), gpu_index_allocation)
            .unwrap();
        assert_eq!(
            mesh_attributes.vertex_positions(),
            &vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(2.0, 0.0, 0.0),
            ]
        );
        assert_eq!(
            mesh_attributes.vertex_normals(),
            &vec![
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
            ]
        );
        assert_eq!(mesh_attributes.indices(), Some(&vec![0, 1, 2]));
        assert_eq!(mesh_attributes.debug_info.name(), "my_mesh");
    }

    #[test]
    fn vertex_positions_missing() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let result = MeshAttributes::builder().build(Handle::zero(), gpu_index_allocation);
        assert_eq!(result, Err(Error::MandatoryAttributeMissing(AttributeType::Positions)));
    }

    #[test]
    fn vertex_normals_missing() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let result = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .build(Handle::zero(), gpu_index_allocation);
        assert_eq!(result, Err(Error::MandatoryAttributeMissing(AttributeType::Normals)));
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
        assert_eq!(result, Err(Error::WrongSize { expected: 3, got: 1 }));
    }

    #[test]
    fn wrong_index() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let result = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)])
            .with_indices(vec![1])
            .build(Handle::zero(), gpu_index_allocation);
        assert_eq!(
            result,
            Err(Error::WrongIndex {
                index_index: 0,
                index_value: 1
            })
        );
    }

    #[test]
    fn wrong_global_index() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let result = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)])
            .with_meshlets(vec![Meshlet {
                global_indices: vec![1], // this vertex doesn't exist
                local_indices: vec![[0, 0, 0]],
            }])
            .build(Handle::zero(), gpu_index_allocation);
        assert_eq!(
            result,
            Err(Error::WrongGlobalMeshletIndex {
                meshlet_index: 0,
                index_index: 0,
                index_value: 1
            })
        );
    }

    #[test]
    fn wrong_local_index() {
        let gpu_index_allocation = GpuIndexAllocation::new_unchecked(0);
        let result = MeshAttributes::builder()
            .with_vertex_positions(vec![Vector3::new(0.0, 0.0, 0.0)])
            .with_vertex_normals(vec![Vector3::new(0.0, 1.0, 0.0)])
            .with_meshlets(vec![Meshlet {
                global_indices: vec![0],        // this vertex exists
                local_indices: vec![[1, 1, 1]], // this index doesn't exist in the global indices
            }])
            .build(Handle::zero(), gpu_index_allocation);
        assert_eq!(
            result,
            Err(Error::WrongLocalMeshletIndex {
                meshlet_index: 0,
                triangle_index: 0,
                index_value: 1
            })
        );
    }
}
