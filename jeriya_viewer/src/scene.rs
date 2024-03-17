use std::{
    io,
    path::{Path, PathBuf},
};

use jeriya_shared::{nalgebra::Vector3, serde_yaml};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RigidMeshInstance {
    pub path: PathBuf,
    pub position: Vector3<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Scene {
    pub rigid_mesh_instances: Vec<RigidMeshInstance>,
}

impl Scene {
    /// Import a scene from the given `path`.
    pub fn import(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)?;
        serde_yaml::from_reader(file).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
