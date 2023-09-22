use std::sync::{mpsc::Sender, Arc};

use jeriya_shared::{
    debug_info, derive_new::new, nalgebra::Vector3, parking_lot::Mutex, thiserror, AsDebugInfo, DebugInfo, EventQueue, Handle,
    IndexingContainer,
};

use crate::{Resource, ResourceEvent};

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
    resource_event_sender: Sender<ResourceEvent>,
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
            gpu_state: InanimateMeshGpuState::WaitingForUpload { vertex_positions, indices },
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
                vertex_posisions: Arc::new(vertex_positions),
            }]))
            .expect("resource event cannot be sent");
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
    pub inanimate_meshes: Arc<Mutex<IndexingContainer<Arc<InanimateMesh>>>>,
    pub resource_event_sender: Sender<ResourceEvent>,
}

impl InanimateMeshGroup {
    pub fn new(event_queue: Sender<ResourceEvent>) -> Self {
        Self {
            resource_event_sender: event_queue,
            inanimate_meshes: Arc::new(Mutex::new(IndexingContainer::new())),
        }
    }

    /// Returns a [`InanimateMeshBuilder`] with the given [`MeshType`] and vertices
    pub fn create(&self, ty: MeshType, vertex_positions: Vec<Vector3<f32>>) -> InanimateMeshBuilder {
        InanimateMeshBuilder::new(self, ty, vertex_positions, self.resource_event_sender.clone())
    }

    /// Inserts a [`InanimateMesh`] into the [`InanimateMeshGroup`]
    fn insert(&self, inanimate_mesh: Arc<InanimateMesh>) {
        insert_inanimate_mesh(inanimate_mesh, self.inanimate_meshes.clone(), self.resource_event_sender.clone());
    }
}

/// This function inserts the given [`InanimateMesh`] into the [`IndexingContainer`] and pushes
/// an [`InanimateMeshEvent::Insert`] into the [`EventQueue`]. This function is used by the
/// [`InanimateMeshGroup`] as well as the [`ModelGroup`] which also operates on [`InanimateMesh`]es.
pub(crate) fn insert_inanimate_mesh(
    inanimate_mesh: Arc<InanimateMesh>,
    inanimate_meshes: Arc<Mutex<IndexingContainer<Arc<InanimateMesh>>>>,
    resource_event_sender: Sender<ResourceEvent>,
) -> Handle<Arc<InanimateMesh>> {
    match &inanimate_mesh.gpu_state {
        InanimateMeshGpuState::WaitingForUpload { vertex_positions, indices } => {
            resource_event_sender
                .send(ResourceEvent::InanimateMesh(vec![InanimateMeshEvent::Insert {
                    inanimate_mesh: inanimate_mesh.clone(),
                    vertex_positions: vertex_positions.clone(),
                    indices: indices.clone(),
                }]))
                .expect("resource event cannot be sent");
        }
        InanimateMeshGpuState::Uploaded { .. } => {
            panic!("InanimateMeshes that are already uploaded are not allowed to be inserted into the InanimateMeshGroup");
        }
    }
    let handle = inanimate_meshes.lock().insert(inanimate_mesh.clone());
    *inanimate_mesh.handle.lock() = Some(handle.clone());
    handle
}

#[derive(new)]
pub struct InanimateMeshBuilder<'a> {
    inanimate_mesh_group: &'a InanimateMeshGroup,
    ty: MeshType,
    vertex_positions: Vec<Vector3<f32>>,
    #[new(default)]
    indices: Option<Vec<u32>>,
    #[new(default)]
    debug_info: Option<DebugInfo>,
    resource_event_sender: Sender<ResourceEvent>,
}

impl<'a> InanimateMeshBuilder<'a> {
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
            self.resource_event_sender,
        )?;
        self.inanimate_mesh_group.insert(inanimate_mesh.clone());
        Ok(inanimate_mesh)
    }
}
