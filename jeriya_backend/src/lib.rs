mod backend;
mod camera;
mod elements;
pub mod immediate;
mod instances;
mod resources;
mod transactions;

pub use backend::*;
pub use camera::*;
pub use elements::*;
pub use instances::*;
use jeriya_shared::{thiserror, winit::window::WindowId};
pub use resources::*;
pub use transactions::*;

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
    #[error("Error concerning the Models")]
    Model(resources::model::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
