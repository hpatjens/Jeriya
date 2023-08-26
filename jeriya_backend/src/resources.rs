pub mod inanimate_mesh;
mod texture2d;

pub use inanimate_mesh::InanimateMesh;
pub use texture2d::*;

use jeriya_shared::AsDebugInfo;

/// Data on the GPU that doesn't change frequently and is referenced by the instances in the scene
pub trait Resource: AsDebugInfo {}
