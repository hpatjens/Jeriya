use std::sync::Arc;

use jeriya_shared::{
    debug_info, nalgebra::Vector3, parking_lot::Mutex, thiserror, AsDebugInfo, DebugInfo, EventQueue, Handle, IndexingContainer,
};

use crate::Resource;

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
    event_queue: Arc<Mutex<EventQueue<InanimateMeshEvent>>>,
    gpu_state: InanimateMeshGpuState,
    handle: Mutex<Option<Handle<Arc<InanimateMesh>>>>,
}

impl InanimateMesh {
    pub(crate) fn new(
        ty: MeshType,
        allocation_type: ResourceAllocationType,
        vertex_positions: Arc<Vec<Vector3<f32>>>,
        indices: Option<Arc<Vec<u32>>>,
        debug_info: DebugInfo,
        event_queue: Arc<Mutex<EventQueue<InanimateMeshEvent>>>,
    ) -> crate::Result<Arc<Self>> {
        check_divisible_vertices_len(vertex_positions.len(), ty)?;
        if let Some(indices) = &indices {
            check_indices(vertex_positions.len(), indices)?;
        }
        let result = Arc::new(Self {
            debug_info,
            event_queue,
            ty,
            allocation_type,
            vertices_len: vertex_positions.len(),
            indices_len: indices.as_ref().map(|indices| indices.len()),
            gpu_state: InanimateMeshGpuState::WaitingForUpload { vertex_positions, indices },
            handle: Mutex::new(None),
        });
        Ok(result)
    }

    /// Sets the vertex positions of the [`InanimateMesh`]
    pub fn set_vertex_positions(self: Arc<Self>, vertex_positions: Vec<Vector3<f32>>) -> crate::Result<()> {
        check_divisible_vertices_len(vertex_positions.len(), self.ty)?;
        if self.allocation_type == ResourceAllocationType::Static && vertex_positions.len() != self.vertices_len {
            return Err(Error::WrongSize {
                expected: self.vertices_len,
                got: vertex_positions.len(),
            }
            .into());
        }
        self.event_queue.lock().push(InanimateMeshEvent::SetVertexPositions {
            inanimate_mesh: self.clone(),
            vertex_posisions: Arc::new(vertex_positions),
        });
        Ok(())
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

pub enum InanimateMeshEvent {
    Insert {
        inanimate_mesh: Arc<InanimateMesh>,
        vertex_positions: Arc<Vec<Vector3<f32>>>,
        indices: Option<Arc<Vec<u32>>>,
    },
    SetVertexPositions {
        inanimate_mesh: Arc<InanimateMesh>,
        vertex_posisions: Arc<Vec<Vector3<f32>>>,
    },
}

pub struct InanimateMeshGroup {
    inanimate_meshes: Arc<Mutex<IndexingContainer<Arc<InanimateMesh>>>>,
    event_queue: Arc<Mutex<EventQueue<InanimateMeshEvent>>>,
}

impl InanimateMeshGroup {
    pub fn new(event_queue: Arc<Mutex<EventQueue<InanimateMeshEvent>>>) -> Self {
        Self {
            event_queue,
            inanimate_meshes: Arc::new(Mutex::new(IndexingContainer::new())),
        }
    }

    /// Returns a [`InanimateMeshBuilder`] with the given [`MeshType`] and vertices
    pub fn create(&self, ty: MeshType, vertex_positions: Vec<Vector3<f32>>) -> InanimateMeshBuilder {
        InanimateMeshBuilder::new(self, ty, vertex_positions, self.event_queue.clone())
    }

    /// Inserts a [`InanimateMesh`] into the [`InanimateMeshGroup`]
    fn insert(&self, inanimate_mesh: Arc<InanimateMesh>) {
        match &inanimate_mesh.gpu_state {
            InanimateMeshGpuState::WaitingForUpload { vertex_positions, indices } => {
                self.event_queue.lock().push(InanimateMeshEvent::Insert {
                    inanimate_mesh: inanimate_mesh.clone(),
                    vertex_positions: vertex_positions.clone(),
                    indices: indices.clone(),
                });
            }
            InanimateMeshGpuState::Uploaded { .. } => {
                panic!("InanimateMeshes that are already uploaded are not allowed to be inserted into the InanimateMeshGroup");
            }
        }
        let handle = self.inanimate_meshes.lock().insert(inanimate_mesh.clone());
        *inanimate_mesh.handle.lock() = Some(handle);
    }
}

pub struct InanimateMeshBuilder<'a> {
    inanimate_mesh_group: &'a InanimateMeshGroup,
    ty: MeshType,
    vertex_positions: Vec<Vector3<f32>>,
    indices: Option<Vec<u32>>,
    debug_info: Option<DebugInfo>,
    event_queue: Arc<Mutex<EventQueue<InanimateMeshEvent>>>,
}

impl<'a> InanimateMeshBuilder<'a> {
    fn new(
        inanimate_mesh_group: &'a InanimateMeshGroup,
        ty: MeshType,
        vertices: Vec<Vector3<f32>>,
        event_queue: Arc<Mutex<EventQueue<InanimateMeshEvent>>>,
    ) -> Self {
        Self {
            inanimate_mesh_group,
            ty,
            vertex_positions: vertices,
            indices: None,
            debug_info: None,
            event_queue,
        }
    }

    /// Sets the indices of the [`InanimateMesh`]
    pub fn with_indices(mut self, indices: Vec<u32>) -> Self {
        self.indices = Some(indices);
        self
    }

    /// Sets the debug info of the [`InanimateMesh`]
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Builds the [`InanimateMesh`] and returns it
    pub fn build(self) -> crate::Result<Arc<InanimateMesh>> {
        let inanimate_mesh = InanimateMesh::new(
            self.ty,
            ResourceAllocationType::Static,
            Arc::new(self.vertex_positions),
            self.indices.map(|indices| Arc::new(indices)),
            self.debug_info.unwrap_or_else(|| debug_info!("Anonymous InanimateMesh")),
            self.event_queue,
        )?;
        self.inanimate_mesh_group.insert(inanimate_mesh.clone());
        Ok(inanimate_mesh)
    }
}
