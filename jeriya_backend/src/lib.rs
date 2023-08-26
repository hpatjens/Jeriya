mod backend;
mod camera;
pub mod immediate;
mod objects;
mod resources;

pub use backend::*;
pub use camera::*;
use jeriya_shared::{thiserror, winit::window::WindowId};
pub use objects::*;
pub use resources::*;

/// Error type for the whole library
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No windows are given")]
    ExpectedWindow,
    #[error("The given window id is not known")]
    UnknownWindowId(WindowId),
    #[error("The maximum capacity of elements is reached: {0}")]
    MaximumCapacityReached(usize),
    #[error("Error from the backend: {0}")]
    Backend(Box<dyn std::error::Error + Send + Sync>),
    #[error("Error concerning the InanimateMeshes")]
    InanimateMesh(resources::inanimate_mesh::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
