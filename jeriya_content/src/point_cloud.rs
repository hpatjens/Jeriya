pub mod bounding_box;
pub mod cluster_hash_grid;
pub mod simple_point_cloud;

use std::{collections::HashSet, fs::File, io::Write, path::Path};

use jeriya_shared::{
    kdtree::KdTree,
    log::{info, trace, warn},
    nalgebra::Vector3,
    ByteColor3,
};
use serde::{Deserialize, Serialize};

use crate::{
    model::Model,
    point_cloud::cluster_hash_grid::{CellContent, CellType, ClusterHashGrid},
};

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
    simple_point_cloud: SimplePointCloud,
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

        Self { simple_point_cloud, pages }
    }

    /// Computes the cluster for the [`PointCloud`] when it contains a [`SimplePointCloud`].
    pub fn compute_clusters(&mut self) {
        let target_points_per_cell = 256;
        let hash_grid = ClusterHashGrid::new(self.simple_point_cloud.point_positions(), target_points_per_cell);

        // Creates a priority queue that is used to process the cells starting from the lowest levels.
        let mut priority_queue = create_priority_queue(&hash_grid);

        // When cells are inserted into a page, they have to be removed from the queue. Instead
        // of searching throught the Vec, the unique index of the cell is stored in a HashSet.
        // Every time a cell is popped from the queue, the HashSet can be used to look up whether
        // the cell still needs processing.
        let mut processed_cells_indices = HashSet::new();

        // The information from the cells is collected into the last page and when it is full,
        // a new page is created.
        let mut pages = vec![Page::default()];

        // Take the cells from the queue and collect them into pages by sampling the neighboring
        // cells until the page is full.
        while let Some(pivot_cell) = priority_queue.pop() {
            if processed_cells_indices.contains(&pivot_cell.unique_index) {
                continue;
            }

            // Insert the pivot cell into the page
            insert_into_page(
                &mut pages,
                &pivot_cell.points,
                self.simple_point_cloud.point_positions(),
                self.simple_point_cloud.point_colors(),
            );

            // Sample the neighboring cells and insert them into the page
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        if x == 0 && y == 0 && z == 0 {
                            continue;
                        }
                        let neighbor_sample = Vector3::new(
                            pivot_cell.center.x + x as f32 * pivot_cell.size.x,
                            pivot_cell.center.y + y as f32 * pivot_cell.size.y,
                            pivot_cell.center.z + z as f32 * pivot_cell.size.z,
                        );
                        if let Some((unique_index, points)) = hash_grid.get_leaf(neighbor_sample) {
                            insert_into_page(
                                &mut pages,
                                points,
                                self.simple_point_cloud.point_positions(),
                                self.simple_point_cloud.point_colors(),
                            );
                            processed_cells_indices.insert(unique_index);
                        }
                    }
                }
            }
        }

        self.pages = pages;
    }

    /// Returns a reference to the [`SimplePointCloud`]. The [`PointCloud`] might not
    /// contain a [`SimplePointCloud`] when only the [`Cluster`]s are used.
    pub fn simple_point_cloud(&self) -> &SimplePointCloud {
        &self.simple_point_cloud
    }

    /// Returns a reference to the [`Page`]s of the [`PointCloud`].
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Writes the point cloud as an OBJ file.
    pub fn to_obj(&self, config: &ObjWriteConfig, obj_writer: impl Write) -> crate::Result<()> {
        match &config.source {
            ObjWriteSource::SimplePointCloud => {
                self.simple_point_cloud.to_obj(obj_writer, config.point_size)?;
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

struct LeafCell {
    unique_index: usize,
    center: Vector3<f32>,
    size: Vector3<f32>,
    points: Vec<usize>,
}

/// Creates a priority queue that is used to process the cells starting from the lowest levels.
fn create_priority_queue(hash_grid: &ClusterHashGrid) -> Vec<LeafCell> {
    // Its not trivial to find the neighbors of a cell in the hash grid because the
    // cells might be subdivided finding the neighbors in the child cells would
    // require knowledge about the direction of neighborhood. However, when traversing
    // down to the leafs, finding the neighbors is trivial because the neighboring
    // cell is always found a cell width aways from the center. Here, the tree is
    // traversed down to the leafs and the cells are inserted into a queue that is
    // used to process the cells starting from the lowest levels.
    let mut priority_queue = Vec::new();
    fn insert_into_queue(priority_queue: &mut Vec<LeafCell>, cell_content: &CellContent) {
        match &cell_content.ty {
            CellType::Points(points) => {
                priority_queue.push(LeafCell {
                    unique_index: cell_content.unique_index,
                    center: cell_content.center,
                    size: cell_content.size,
                    points: points.clone(),
                });
            }
            CellType::Grid(grid) => insert_into_queue_from_grid(priority_queue, grid.cells()),
            CellType::XAxisHalfSplit(first, second) => {
                insert_into_queue(priority_queue, first);
                insert_into_queue(priority_queue, second);
            }
        }
    }
    fn insert_into_queue_from_grid<'a>(priority_queue: &mut Vec<LeafCell>, cells: impl Iterator<Item = &'a CellContent>) {
        // Since we want to traverse the tree donw to the leafs before adding cells in the
        // higher levels, the cells are partitioned into recursive and non-recursive cells.
        let (recursive, non_recursive) = cells.partition::<Vec<_>, _>(|cell| matches!(&cell.ty, CellType::Grid(_)));
        for cell in recursive {
            insert_into_queue(priority_queue, cell);
        }
        for cell in non_recursive {
            insert_into_queue(priority_queue, cell);
        }
    }
    insert_into_queue_from_grid(&mut priority_queue, hash_grid.cells());
    priority_queue
}

/// Inserts the points into the page.
fn insert_into_page(pages: &mut Vec<Page>, points: &[usize], point_positions: &[Vector3<f32>], point_colors: &[ByteColor3]) {
    // When the last page is full, a new page is created.
    let has_space = pages.last().map_or(false, |page| page.clusters.len() + 1 < Page::MAX_CLUSTERS);
    if !has_space {
        pages.push(Page::default());
    }

    // Insert the cluster into the page
    let Some(last_page) = pages.last_mut() else {
        panic!("Failed to get the last page");
    };
    let index_start = last_page.point_positions.len() as u32;
    let len = points.len() as u32;
    let cluster = Cluster { index_start, len };
    last_page.clusters.push(cluster);
    last_page.point_positions.extend(points.iter().map(|&index| point_positions[index]));
    last_page.point_colors.extend(points.iter().map(|&index| point_colors[index]));
}
