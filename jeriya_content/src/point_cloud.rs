pub mod bounding_box;
pub mod cluster_hash_grid;
pub mod simple_point_cloud;

use std::{fs::File, io::Write, path::Path};

use jeriya_shared::{
    kdtree::KdTree,
    log::{info, trace, warn},
    nalgebra::Vector3,
    rayon::iter::IndexedParallelIterator,
    ByteColor3,
};
use serde::{Deserialize, Serialize};

use crate::{model::Model, point_cloud::cluster_hash_grid::ClusterHashGrid};

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
    pub const MAX_CLUSTERS: usize = 16;
    /// The maximum number of points a page can have.
    pub const MAX_POINTS: usize = Cluster::MAX_POINTS * Self::MAX_CLUSTERS;
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

#[derive(Default, Clone, Serialize, Deserialize)]
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

        fn to_array(v: &Vector3<f32>) -> [f32; 3] {
            [v.x, v.y, v.z]
        }

        // Insert all points into the KdTree
        let mut kdtree = KdTree::with_capacity(3, simple_point_cloud.point_positions().len());
        for (index, point) in simple_point_cloud.point_positions().iter().enumerate() {
            kdtree.add(to_array(point), index).expect("Failed to insert into KdTree");
        }

        let mut pages = vec![Page::default()];
        let mut point_count = 0;

        // Extracts the next cluster from the given [`KdTree`] meaning that the points belonging
        // to the cluster are removed from the [`KdTree`] and returned in the [`Cluster`].
        let mut extract_next_cluster = || -> bool {
            fn distance(a: &[f32], b: &[f32]) -> f32 {
                let dx = a[0] - b[0];
                let dy = a[1] - b[1];
                let dz = a[2] - b[2];
                dx * dx + dy * dy + dz * dz
            }

            // Select the point that is nearest to the origin
            let pivot_point = kdtree.nearest(&[0.0, 0.0, 0.0], 1, &distance);
            let Ok(pivot_point) = pivot_point else {
                warn!("Failed to get pivot point");
                return false;
            };
            let Some(pivot_point_position) = pivot_point
                .first()
                .map(|(_disance, &index)| to_array(&simple_point_cloud.point_positions()[index]))
            else {
                trace!("No points left for the Cluster");
                return false;
            };

            // Select the points that are nearest to the pivot point
            let nearest_iter = kdtree
                .iter_nearest(&pivot_point_position, &distance)
                .expect("Failed to get nearest iterator");
            let cluster_points = nearest_iter
                .take(Cluster::MAX_POINTS)
                .map(|(_distance, &index)| index)
                .collect::<Vec<_>>();

            // Check if the page can hold the cluster
            let current_page = {
                let current_page = pages.last_mut().expect("Failed to get the current page");
                let would_overflow_points = current_page.point_positions.len() + cluster_points.len() > Page::MAX_CLUSTERS;
                let would_overflow_clusters = current_page.clusters.len() + 1 > Page::MAX_CLUSTERS;
                if would_overflow_points || would_overflow_clusters {
                    pages.push(Page::default());
                }
                pages.last_mut().expect("Failed to get the current page")
            };

            // Insert the points into the page
            let index_start = current_page.point_positions.len() as u32;
            for index in cluster_points.iter() {
                let point = simple_point_cloud.point_positions()[*index];
                let color = simple_point_cloud.point_colors()[*index];
                current_page.point_positions.push(point);
                current_page.point_colors.push(color);
            }

            // Insert the cluster into the page
            let cluster = Cluster {
                index_start,
                len: cluster_points.len() as u32,
            };
            point_count += cluster.len as usize;
            trace!(
                "Extracted cluster with {} points ({}%)",
                cluster.len,
                point_count as f32 / simple_point_cloud.point_positions().len() as f32 * 100.0
            );
            current_page.clusters.push(cluster);

            // Remove the points from the KdTree
            for index in cluster_points {
                let point = simple_point_cloud.point_positions()[index];
                kdtree.remove(&to_array(&point), &index).expect("Failed to remove from KdTree");
            }

            true
        };

        // Extracts all clusters from the [`KdTree`].
        while extract_next_cluster() {}
        info! {
            "Extracted {} clusters in {} pages",
            pages.iter().map(|page| page.clusters.len()).sum::<usize>(),
            pages.len()
        }

        Self {
            simple_point_cloud: Some(simple_point_cloud),
            pages,
        }
    }

    pub fn compute_clusters(&mut self) {
        if let Some(simple_point_cloud) = &self.simple_point_cloud {
            let target_points_per_cell = 128;
            let hash_grid = ClusterHashGrid::new(simple_point_cloud.point_positions(), target_points_per_cell);

            todo!()
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

impl std::fmt::Debug for PointCloud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointCloud")
            .field("simple_point_cloud", &self.simple_point_cloud)
            .field("pages", &self.pages.len())
            .finish()
    }
}
