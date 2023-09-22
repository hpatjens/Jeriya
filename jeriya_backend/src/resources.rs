pub mod inanimate_mesh;
pub mod model;
mod texture2d;

pub use inanimate_mesh::InanimateMesh;
pub use model::Model;
pub use texture2d::*;

use jeriya_shared::AsDebugInfo;

use self::inanimate_mesh::InanimateMeshEvent;

/// Data on the GPU that doesn't change frequently and is referenced by the instances in the scene
pub trait Resource: AsDebugInfo {}

/// Event that is sent to the resource thread to update the resources
pub enum ResourceEvent {
    FrameStart,
    InanimateMesh(Vec<InanimateMeshEvent>),
}
