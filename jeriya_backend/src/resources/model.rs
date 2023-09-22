use std::sync::{mpsc::Sender, Arc};

use jeriya_shared::{debug_info, derive_new::new, parking_lot::Mutex, thiserror, DebugInfo, EventQueue, Handle, IndexingContainer};

use crate::{
    inanimate_mesh::{insert_inanimate_mesh, InanimateMeshEvent, InanimateMeshGroup, MeshType, ResourceAllocationType},
    InanimateMesh, ResourceEvent,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {}

impl From<Error> for crate::Error {
    fn from(error: Error) -> Self {
        crate::Error::Model(error)
    }
}

pub enum ModelSource {
    Model(jeriya_content::model::Model),
}

impl From<jeriya_content::model::Model> for ModelSource {
    fn from(model: jeriya_content::model::Model) -> Self {
        Self::Model(model)
    }
}

/// Model that groups a set of [`InanimateMesh`]es together.
#[derive(new, Debug)]
pub struct Model {
    debug_info: DebugInfo,
    inanimate_meshes: Vec<Handle<Arc<InanimateMesh>>>,
}

impl Model {
    pub fn inanimate_meshes(&self) -> &[Handle<Arc<InanimateMesh>>] {
        self.inanimate_meshes.as_ref()
    }
}

/// Manages a group of [`Model`]s.
pub struct ModelGroup {
    models: Arc<Mutex<Vec<Arc<Model>>>>,

    // These are the inanimate meshes that are managed by the [`InanimateMeshGroup`]. They are
    // used here to create [`InanimateMesh`]es for the models meshes as long as the renderer
    // doesn't support models on the GPU.
    inanimate_meshes: Arc<Mutex<IndexingContainer<Arc<InanimateMesh>>>>,
    resource_event_sender: Sender<ResourceEvent>,
}

impl ModelGroup {
    pub fn new(inanimate_mesh_group: &InanimateMeshGroup) -> Self {
        Self {
            models: Arc::new(Mutex::new(Vec::new())),
            inanimate_meshes: inanimate_mesh_group.inanimate_meshes.clone(),
            resource_event_sender: inanimate_mesh_group.resource_event_sender.clone(),
        }
    }
}

impl ModelGroup {
    pub fn create(&self, model_source: impl Into<ModelSource>) -> ModelBuilder {
        ModelBuilder::new(
            self,
            self.inanimate_meshes.clone(),
            self.resource_event_sender.clone(),
            model_source.into(),
        )
    }
}

#[derive(new)]
pub struct ModelBuilder<'a> {
    _model_group: &'a ModelGroup,
    inanimate_meshes: Arc<Mutex<IndexingContainer<Arc<InanimateMesh>>>>,
    resource_event_sender: Sender<ResourceEvent>,
    model_source: ModelSource,
    #[new(default)]
    debug_info: Option<DebugInfo>,
}

impl<'a> ModelBuilder<'a> {
    /// Sets the debug info for the [`Model`]
    pub fn with_debug_info(mut self, debug_info: DebugInfo) -> Self {
        self.debug_info = Some(debug_info);
        self
    }

    /// Builds the [`Model`] and returns it
    pub fn build(self) -> crate::Result<Arc<Model>> {
        // Create the inanimate meshes for the model
        match &self.model_source {
            ModelSource::Model(model) => {
                let mut inanimate_meshes = Vec::new();
                for (mesh_index, mesh) in model.meshes.iter().enumerate() {
                    let vertex_positions = Arc::new(mesh.simple_mesh.vertex_positions.clone());
                    let indices = Some(Arc::new(mesh.simple_mesh.indices.clone()));
                    let inanimate_mesh = InanimateMesh::new(
                        MeshType::TriangleList,
                        ResourceAllocationType::Static,
                        vertex_positions.clone(),
                        indices.clone(),
                        debug_info!(format!("InanimateMesh {} of Model {}", mesh_index, model.name)),
                        self.resource_event_sender.clone(),
                    )?;
                    let handle = insert_inanimate_mesh(inanimate_mesh, self.inanimate_meshes.clone(), self.resource_event_sender.clone());
                    inanimate_meshes.push(handle);
                }

                let model = Arc::new(Model::new(
                    self.debug_info.unwrap_or_else(|| debug_info!("Anonymous Model")),
                    inanimate_meshes,
                ));
                Ok(model)
            }
        }
    }
}
