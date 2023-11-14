pub mod simple_point_cloud;

use std::{fs::File, io::Write, path::Path};

use jeriya_shared::{nalgebra::Vector3, ByteColor3};
use serde::{Deserialize, Serialize};

use crate::model::Model;

use self::simple_point_cloud::SimplePointCloud;

/// Configuration for writing an OBJ file.
pub struct ObjWriteConfig {
    pub source: ObjWriteSource,
    pub point_size: f32,
}

/// Determines whether the [`SimplePointCloud`] or the [`Cluster`]s are used to produce the OBJ file.
pub enum ObjWriteSource {
    SimplePointCloud,
    Clusters,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    point_positions: Vec<Vector3<f32>>,
    point_colors: Vec<ByteColor3>,
    clusters: Vec<Cluster>,
}

impl Page {
    /// The maximum number of clusters a page can have.
    pub const MAX_CLUSTERS: usize = 16 * Cluster::MAX_POINTS;
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// Index into the `Page`'s `point_positions` and `point_colors` `Vec`s
    index_start: u32,
    /// Number of points in the cluster
    len: u32,
}

impl Cluster {
    /// The maximum number of points a cluster can have.
    pub const MAX_POINTS: usize = 128;
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PointCloud {
    simple_point_cloud: Option<SimplePointCloud>,
    pages: Vec<Page>,
}

impl PointCloud {
    /// Creates a new empty [`PointCloud`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new [`PointCloud`] from the given [`Model`].
    pub fn sample_from_model(model: &Model, points_per_square_unit: f32) -> Self {
        let simple_point_cloud = SimplePointCloud::sample_from_model(model, points_per_square_unit);
        let pages = vec![Page::default()];
        Self {
            simple_point_cloud: Some(simple_point_cloud),
            pages,
        }
    }

    /// Returns a reference to the [`SimplePointCloud`]. The [`PointCloud`] might not
    /// contain a [`SimplePointCloud`] when only the [`Cluster`]s are used.
    pub fn simple_point_cloud(&self) -> Option<&SimplePointCloud> {
        self.simple_point_cloud.as_ref()
    }

    /// Returns a reference to the [`Page`]s of the [`PointCloud`].
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Writes the point cloud as an OBJ file.
    pub fn to_obj(&self, config: &ObjWriteConfig, obj_writer: impl Write) -> crate::Result<()> {
        match &config.source {
            ObjWriteSource::SimplePointCloud => {
                if let Some(simple_point_cloud) = &self.simple_point_cloud {
                    simple_point_cloud.to_obj(obj_writer, config.point_size)?;
                }
                Ok(())
            }
            ObjWriteSource::Clusters => todo!(),
        }
    }

    /// Serializes the `PointCloud` to a file.
    pub fn serialize_to_file(&self, filepath: &impl AsRef<Path>) -> crate::Result<()> {
        let file = File::create(filepath)?;
        bincode::serialize_into(file, self).map_err(|err| crate::Error::FailedSerialization(err))?;
        Ok(())
    }

    /// Deserializes the `PointCloud` from a file.
    pub fn deserialize_from_file(filepath: &impl AsRef<Path>) -> crate::Result<Self> {
        let file = File::open(filepath)?;
        bincode::deserialize_from(file).map_err(|err| crate::Error::FailedDeserialization(err))
    }
}
