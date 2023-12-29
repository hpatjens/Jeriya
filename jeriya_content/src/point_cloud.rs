pub mod cluster_graph;
pub mod clustered_point_cloud;
pub mod point_clustering_hash_grid;
pub mod point_clustering_octree;
pub mod simple_point_cloud;

use jeriya_shared::aabb::AABB;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    fs::File,
    io::{self, Write},
    path::Path,
};

use crate::model::Model;

use self::{
    clustered_point_cloud::{ClusteredPointCloud, ObjClusterWriteConfig},
    simple_point_cloud::SimplePointCloud,
};

/// Configuration for writing an OBJ file.
pub enum ObjWriteConfig {
    SimplePointCloud(simple_point_cloud::ObjWriteConfig),
    Clusters(ObjClusterWriteConfig),
}

/// Determines whether the [`SimplePointCloud`] or the [`Cluster`]s are used to produce the OBJ file.
pub enum ObjWriteSource {
    SimplePointCloud,
    Clusters,
}

/// Information for debugging that is recorded during the creation of the [`PointCloud`].
#[derive(Clone, Serialize, Deserialize)]
pub struct DebugGeometry {
    hash_grid_cells: Vec<AABB>,
}

/// A `PointCloud` is a collection of points representing the surface of objects.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PointCloud {
    simple_point_cloud: SimplePointCloud,
    clustered_point_cloud: Option<ClusteredPointCloud>,
}

impl PointCloud {
    /// Creates a new empty [`PointCloud`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new [`PointCloud`] from the given [`Model`].
    pub fn sample_from_model(model: &Model, points_per_square_unit: f32, debug_directory: Option<&Path>) -> Self {
        let simple_point_cloud = SimplePointCloud::sample_from_model(model, points_per_square_unit);
        let clustered_point_cloud = ClusteredPointCloud::from_simple_point_cloud(&simple_point_cloud, debug_directory);
        Self {
            simple_point_cloud,
            clustered_point_cloud: Some(clustered_point_cloud),
        }
    }

    /// Returns a reference to the [`SimplePointCloud`]. The [`PointCloud`] might not
    /// contain a [`SimplePointCloud`] when only the [`Cluster`]s are used.
    pub fn simple_point_cloud(&self) -> &SimplePointCloud {
        &self.simple_point_cloud
    }

    /// Returns a reference to the [`Page`]s of the [`PointCloud`].
    pub fn clustered_point_cloud(&self) -> Option<&ClusteredPointCloud> {
        self.clustered_point_cloud.as_ref()
    }

    /// Writes the point cloud as an OBJ file.
    pub fn to_obj(&self, config: &ObjWriteConfig, obj_writer: impl Write, mtl_writer: impl Write, mtl_filename: &str) -> io::Result<()> {
        match &config {
            ObjWriteConfig::SimplePointCloud(obj_write_config) => self.simple_point_cloud.to_obj(obj_writer, obj_write_config),
            ObjWriteConfig::Clusters(obj_cluster_write_config) => {
                if let Some(clustered_point_cloud) = &self.clustered_point_cloud {
                    clustered_point_cloud.to_obj(obj_writer, mtl_writer, mtl_filename, obj_cluster_write_config)
                } else {
                    panic!("Failed to write obj for clustered point cloud. The clustered point cloud is not initialized.");
                }
            }
        }
    }

    /// Writes the point cloud as an OBJ file. The MTL file is written to the same directory.
    pub fn to_obj_file(&self, config: &ObjWriteConfig, filepath: &impl AsRef<Path>) -> io::Result<()> {
        let obj_filepath = filepath.as_ref().with_extension("obj");
        let mtl_filepath = filepath.as_ref().with_extension("mtl");
        let obj_file = File::create(&obj_filepath)?;
        let mtl_file = File::create(&mtl_filepath)?;
        let mtl_filename = mtl_filepath
            .file_name()
            .expect("Failed to get MTL filename")
            .to_str()
            .expect("Failed to convert MTL filename to str");
        self.to_obj(config, obj_file, mtl_file, mtl_filename)
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

#[cfg(test)]
mod tests {
    use jeriya_shared::function_name;
    use jeriya_test::create_test_result_folder_for_function;

    use super::*;

    #[test]
    fn test_sample_from_model() {
        env_logger::builder().filter_level(jeriya_shared::log::LevelFilter::Trace).init();

        let directory = create_test_result_folder_for_function(function_name!());

        let model = Model::import("../sample_assets/models/suzanne.glb").unwrap();
        let point_cloud = PointCloud::sample_from_model(&model, 2000.0, Some(&directory));

        if let Some(clustered_point_cloud) = point_cloud.clustered_point_cloud() {
            for depth in 0..clustered_point_cloud.max_cluster_depth() {
                let config = ObjWriteConfig::Clusters(ObjClusterWriteConfig::Points { point_size: 0.02, depth });
                point_cloud
                    .to_obj_file(&config, &directory.join(format!("point_cloud_depth{depth}.obj")))
                    .unwrap();
            }
        }

        let config = ObjWriteConfig::Clusters(ObjClusterWriteConfig::HashGridCells);
        point_cloud
            .to_obj_file(&config, &directory.join("point_cloud_hash_grid_cells.obj"))
            .unwrap();

        let config = ObjWriteConfig::SimplePointCloud(simple_point_cloud::ObjWriteConfig::AABB);
        point_cloud
            .to_obj_file(&config, &directory.join("point_cloud_bounding_box.obj"))
            .unwrap();

        point_cloud
            .clustered_point_cloud()
            .unwrap()
            .plot_cluster_fill_level_histogram(&directory.join("cluster_fill_level_histogram.svg"))
            .unwrap();

        point_cloud
            .clustered_point_cloud()
            .unwrap()
            .plot_page_fill_level_histogram(&directory.join("page_fill_level_histogram.svg"))
            .unwrap();

        point_cloud
            .clustered_point_cloud()
            .unwrap()
            .write_statisics(&directory.join("statistics.json"))
            .unwrap();
    }
}
