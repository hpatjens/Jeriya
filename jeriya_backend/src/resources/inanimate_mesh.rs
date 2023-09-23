use std::sync::{mpsc::Sender, Arc};

use jeriya_shared::{nalgebra::Vector3, parking_lot::Mutex, thiserror, AsDebugInfo, DebugInfo, Handle};

use crate::{Resource, ResourceEvent};

use super::inanimate_mesh_group::InanimateMeshGroup;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The number of vertices doesn't match the allocated size. Expected {expected} but got {got}")]
    WrongSize { expected: usize, got: usize },
    #[error("The number of vertices is not divisible by the number of vertices that is expected due to the MeshType. Expected to be divisible by {denumerator} but got {len} vertices")]
    NonDivisible { len: usize, denumerator: usize },
    #[error("The index {index_index} is out of bounds. The number of vertices is {vertices_len} but the index is {index_value}")]
    WrongIndex {
        vertices_len: usize,
        index_index: usize,
        index_value: u32,
    },
}

impl From<Error> for crate::Error {
    fn from(error: Error) -> Self {
        crate::Error::InanimateMesh(error)
    }
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum ResourceAllocationType {
    #[default]
    Static,
    Dynamic,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum MeshType {
    #[default]
    Points,
    Lines,
    LineList,
    Triangles,
    TriangleList,
}

#[derive(Debug)]
pub enum InanimateMeshGpuState {
    WaitingForUpload {
        vertex_positions: Arc<Vec<Vector3<f32>>>,
        vertex_normals: Arc<Vec<Vector3<f32>>>,
        indices: Option<Arc<Vec<u32>>>,
    },
    Uploaded {
        inanimate_mesh_offset: u64,
    },
}

pub struct InanimateMesh {
    debug_info: DebugInfo,
    ty: MeshType,
    allocation_type: ResourceAllocationType,
    vertices_len: usize,
    indices_len: Option<usize>,
    resource_event_sender: Sender<ResourceEvent>,
    gpu_state: InanimateMeshGpuState,

    /// This is pub(crate) because the [`InanimateMeshGroup`] must be able to set this
    pub(crate) handle: Mutex<Option<Handle<Arc<InanimateMesh>>>>,
}

impl InanimateMesh {
    pub(crate) fn new(
        ty: MeshType,
        allocation_type: ResourceAllocationType,
        vertex_positions: Arc<Vec<Vector3<f32>>>,
        vertex_normals: Arc<Vec<Vector3<f32>>>,
        indices: Option<Arc<Vec<u32>>>,
        debug_info: DebugInfo,
        resource_event_sender: Sender<ResourceEvent>,
    ) -> crate::Result<Arc<Self>> {
        if let Some(indices) = &indices {
            check_indices(vertex_positions.len(), indices)?;
        } else {
            check_divisible_vertices_len(vertex_positions.len(), ty)?;
        }
        let result = Arc::new(Self {
            debug_info,
            resource_event_sender,
            ty,
            allocation_type,
            vertices_len: vertex_positions.len(),
            indices_len: indices.as_ref().map(|indices| indices.len()),
            gpu_state: InanimateMeshGpuState::WaitingForUpload {
                vertex_positions,
                vertex_normals,
                indices,
            },
            handle: Mutex::new(None),
        });
        Ok(result)
    }

    /// Sets the vertex positions of the [`InanimateMesh`]
    pub fn set_vertex_positions(self: Arc<Self>, vertex_positions: Vec<Vector3<f32>>) -> crate::Result<()> {
        if self.allocation_type == ResourceAllocationType::Static && vertex_positions.len() != self.vertices_len {
            return Err(Error::WrongSize {
                expected: self.vertices_len,
                got: vertex_positions.len(),
            }
            .into());
        }
        self.resource_event_sender
            .send(ResourceEvent::InanimateMesh(vec![InanimateMeshEvent::SetVertexPositions {
                inanimate_mesh: self.clone(),
                vertex_positions: Arc::new(vertex_positions),
            }]))
            .expect("resource event cannot be sent");
        Ok(())
    }

    /// Returns the [`MeshType`] of the [`InanimateMesh`]
    pub fn mesh_type(&self) -> MeshType {
        self.ty
    }

    /// Returns the state in which the [`InanimateMesh`] is on the GPU
    pub fn gpu_state(&self) -> &InanimateMeshGpuState {
        &self.gpu_state
    }

    /// Number of vertices in the [`InanimateMesh`]
    pub fn vertices_len(&self) -> usize {
        self.vertices_len
    }

    /// Number of indices in the [`InanimateMesh`]
    pub fn indices_len(&self) -> Option<usize> {
        self.indices_len
    }

    /// Returns the handle of the [`InanimateMesh`] in its [`IndexingContainer`]
    pub fn handle(&self) -> Handle<Arc<InanimateMesh>> {
        self.handle.lock().clone().expect("handle is not initialized")
    }

    /// Returns the debug info of the [`InanimateMesh`]
    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

impl std::fmt::Debug for InanimateMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InanimateMesh")
            .field("debug_info", &self.debug_info)
            .field("ty", &self.ty)
            .field("allocation_type", &self.allocation_type)
            .field("vertices_len", &self.vertices_len)
            .field("indices_len", &self.indices_len)
            .field("gpu_state", &self.gpu_state)
            .field("handle", &self.handle)
            .finish()
    }
}

/// Checks if the number of vertices is divisible by the number of vertices that is expected due to the [`InanimateMeshType`]
fn check_divisible_vertices_len(len: usize, inanimate_mesh_type: MeshType) -> crate::Result<()> {
    match inanimate_mesh_type {
        MeshType::Points => {}
        MeshType::Lines | MeshType::LineList => {
            if len % 2 != 0 {
                return Err(Error::NonDivisible { len, denumerator: 2 }.into());
            }
        }
        MeshType::Triangles | MeshType::TriangleList => {
            if len % 3 != 0 {
                return Err(Error::NonDivisible { len, denumerator: 3 }.into());
            }
        }
    }
    Ok(())
}

/// Checks if the indices are valid for the given number of vertices
fn check_indices(vertices_len: usize, indices: &[u32]) -> crate::Result<()> {
    for (index_index, &index_value) in indices.iter().enumerate() {
        if index_value as usize >= vertices_len {
            return Err(Error::WrongIndex {
                vertices_len,
                index_index,
                index_value,
            }
            .into());
        }
    }
    Ok(())
}

impl Resource for InanimateMesh {}

impl AsDebugInfo for InanimateMesh {
    fn as_debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }
}

#[derive(Debug)]
pub enum InanimateMeshEvent {
    Insert {
        inanimate_mesh: Arc<InanimateMesh>,
        vertex_positions: Arc<Vec<Vector3<f32>>>,
        vertex_normals: Arc<Vec<Vector3<f32>>>,
        indices: Option<Arc<Vec<u32>>>,
    },
    SetVertexPositions {
        inanimate_mesh: Arc<InanimateMesh>,
        vertex_positions: Arc<Vec<Vector3<f32>>>,
    },
}
