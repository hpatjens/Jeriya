pub mod bounding_box;
pub mod cluster_hash_grid;
pub mod simple_point_cloud;

use std::{
    collections::HashSet,
    fs::File,
    io::{self, Write},
    path::Path,
};

use jeriya_shared::{
    colors_transform::{Color, Hsl},
    nalgebra::Vector3,
    rand, ByteColor3,
};
use serde::{Deserialize, Serialize};

use crate::{
    model::Model,
    point_cloud::cluster_hash_grid::{CellContent, CellType, ClusterHashGrid},
};

use self::{bounding_box::AABB, simple_point_cloud::SimplePointCloud};

/// Configuration for writing an OBJ file.
pub enum ObjWriteConfig {
    SimplePointCloud(simple_point_cloud::ObjWriteConfig),
    Clusters { point_size: f32 },
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

#[derive(Clone, Serialize, Deserialize)]
pub struct DebugGeometry {
    hash_grid_cells: Vec<AABB>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PointCloud {
    simple_point_cloud: SimplePointCloud,
    pages: Vec<Page>,
    debug_geometry: Option<DebugGeometry>,
}

impl PointCloud {
    /// Creates a new empty [`PointCloud`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new [`PointCloud`] from the given [`Model`].
    pub fn sample_from_model(model: &Model, points_per_square_unit: f32) -> Self {
        let simple_point_cloud = SimplePointCloud::sample_from_model(model, points_per_square_unit);

        Self {
            simple_point_cloud,
            pages: Vec::new(),
            debug_geometry: None,
        }
    }

    /// Computes the cluster for the [`PointCloud`].
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
    pub fn to_obj(&self, config: &ObjWriteConfig, obj_writer: impl Write, mtl_writer: impl Write, mtl_filename: &str) -> io::Result<()> {
        match &config {
            ObjWriteConfig::SimplePointCloud(obj_write_config) => self.simple_point_cloud.to_obj(obj_writer, obj_write_config),
            ObjWriteConfig::Clusters { point_size } => {
                pages_to_obj(&self.pages, obj_writer, mtl_writer, mtl_filename, *point_size)?;
                Ok(())
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

impl std::fmt::Debug for PointCloud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointCloud")
            .field("simple_point_cloud", &self.simple_point_cloud)
            .field("pages", &self.pages.len())
            .finish()
    }
}

fn pages_to_obj(
    pages: &Vec<Page>,
    mut obj_writer: impl Write,
    mut mtl_writer: impl Write,
    mtl_filename: &str,
    point_size: f32,
) -> io::Result<()> {
    writeln!(obj_writer, "mtllib {mtl_filename}")?;

    // Write OBJ file
    let mut global_clutser_index = 0;
    for (page_index, page) in pages.iter().enumerate() {
        let mut vertex_index = 1;
        for (cluster_index, cluster) in page.clusters.iter().enumerate() {
            writeln!(obj_writer, "")?;
            writeln!(obj_writer, "# Cluster {cluster_index} in page {page_index}")?;
            writeln!(obj_writer, "o cluster_{global_clutser_index}")?;
            writeln!(obj_writer, "usemtl cluster_{global_clutser_index}")?;
            for index in cluster.index_start..cluster.index_start + cluster.len {
                let position = &page.point_positions[index as usize];
                let (a, b, c) = SimplePointCloud::create_triangle_for_point(position, point_size)?;
                writeln!(obj_writer, "v {} {} {}", a.x, a.y, a.z)?;
                writeln!(obj_writer, "v {} {} {}", b.x, b.y, b.z)?;
                writeln!(obj_writer, "v {} {} {}", c.x, c.y, c.z)?;
            }
            for index in cluster.index_start..cluster.index_start + cluster.len {
                let f0 = vertex_index + index * 3;
                let f1 = vertex_index + index * 3 + 1;
                let f2 = vertex_index + index * 3 + 2;
                writeln!(obj_writer, "f {f0} {f1} {f2}")?;
            }
            vertex_index += cluster.len * 3;
            global_clutser_index += 1;
        }
    }

    // Write MTL file
    let mut global_clutser_index = 0;
    for (page_index, page) in pages.iter().enumerate() {
        for (cluster_index, _cluster) in page.clusters.iter().enumerate() {
            let hue = rand::random::<f32>() * 360.0;
            let hsv = Hsl::from(hue, 100.0, 50.0);
            let rgb = hsv.to_rgb();
            let r = rgb.get_red() / 255.0;
            let g = rgb.get_green() / 255.0;
            let b = rgb.get_blue() / 255.0;
            writeln!(mtl_writer, "# Material for cluster {cluster_index} in page {page_index}")?;
            writeln!(mtl_writer, "newmtl cluster_{global_clutser_index}")?;
            writeln!(mtl_writer, "Ka {r} {g} {b}")?;
            writeln!(mtl_writer, "Kd {r} {g} {b}")?;
            writeln!(mtl_writer, "Ks 1.0 1.0 1.0")?;
            writeln!(mtl_writer, "Ns 100")?;
            writeln!(mtl_writer, "")?;
            global_clutser_index += 1;
        }
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use jeriya_shared::function_name;
    use jeriya_test::create_test_result_folder_for_function;

    use super::*;

    #[test]
    fn test_sample_from_model() {
        let model = Model::import("../sample_assets/models/suzanne.glb").unwrap();
        let mut point_cloud = PointCloud::sample_from_model(&model, 200.0);
        point_cloud.compute_clusters();
        dbg!(point_cloud.pages().len());
        dbg!(point_cloud.simple_point_cloud().point_positions().len());
        let directory = create_test_result_folder_for_function(function_name!());
        let config = ObjWriteConfig::Clusters { point_size: 0.02 };
        point_cloud.to_obj_file(&config, &directory.join("point_cloud.obj")).unwrap();
    }
}
