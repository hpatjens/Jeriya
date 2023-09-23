use std::sync::{mpsc::Sender, Arc};

use jeriya_shared::{debug_info, derive_new::new, nalgebra::Vector3, parking_lot::Mutex, DebugInfo, Handle, IndexingContainer};

use crate::{
    inanimate_mesh::{InanimateMeshEvent, InanimateMeshGpuState, MeshType, ResourceAllocationType},
    InanimateMesh, ResourceEvent,
};

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

    /// Returns a [`InanimateMeshBuilder`] with the given [`MeshType`], vertex positions and vertex normals
    pub fn create(&self, ty: MeshType, vertex_positions: Vec<Vector3<f32>>, vertex_normals: Vec<Vector3<f32>>) -> InanimateMeshBuilder {
        InanimateMeshBuilder::new(self, ty, vertex_positions, vertex_normals, self.resource_event_sender.clone())
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
    match inanimate_mesh.gpu_state() {
        InanimateMeshGpuState::WaitingForUpload {
            vertex_positions,
            vertex_normals,
            indices,
        } => {
            resource_event_sender
                .send(ResourceEvent::InanimateMesh(vec![InanimateMeshEvent::Insert {
                    inanimate_mesh: inanimate_mesh.clone(),
                    vertex_positions: vertex_positions.clone(),
                    vertex_normals: vertex_normals.clone(),
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
    vertex_normals: Vec<Vector3<f32>>,
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
            Arc::new(self.vertex_normals),
            self.indices.map(|indices| Arc::new(indices)),
            self.debug_info.unwrap_or_else(|| debug_info!("Anonymous InanimateMesh")),
            self.resource_event_sender,
        )?;
        self.inanimate_mesh_group.insert(inanimate_mesh.clone());
        Ok(inanimate_mesh)
    }
}
