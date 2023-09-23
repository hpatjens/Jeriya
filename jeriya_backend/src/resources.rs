pub mod inanimate_mesh;
pub mod model;
mod texture2d;

pub use inanimate_mesh::InanimateMesh;
pub use model::Model;
pub use texture2d::*;

use jeriya_shared::AsDebugInfo;

use crate::ResourceReceiver;

use self::inanimate_mesh::{InanimateMeshEvent, InanimateMeshGroup};

/// Data on the GPU that doesn't change frequently and is referenced by the instances in the scene
pub trait Resource: AsDebugInfo {}

/// Event that is sent to the resource thread to update the resources
pub enum ResourceEvent {
    FrameStart,
    InanimateMesh(Vec<InanimateMeshEvent>),
}

pub struct ResourceGroup {
    inanimate_mesh_group: InanimateMeshGroup,
}

impl ResourceGroup {
    /// Creates a new [`ResourceGroup`]
    pub fn new(resource_receiver: impl ResourceReceiver) -> Self {
        Self {
            inanimate_mesh_group: InanimateMeshGroup::new(resource_receiver.sender()),
        }
    }

    pub fn inanimate_meshes(&self) -> &InanimateMeshGroup {
        &self.inanimate_mesh_group
    }
}
