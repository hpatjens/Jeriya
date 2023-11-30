use std::{
    collections::HashSet,
    io::{self, Write},
    sync::atomic::AtomicUsize,
};

use jeriya_shared::{
    aabb::AABB,
    colors_transform::{Color, Hsl},
    itertools::Itertools,
    log::info,
    nalgebra::Vector3,
    obj_writer::write_bounding_box_o,
    rand, ByteColor3,
};
use serde::{Deserialize, Serialize};

use crate::point_cloud::cluster_hash_grid::{BoundingBoxStrategy, CellContent, CellType, ClusterHashGrid, Context, Selection};

use super::{simple_point_cloud::SimplePointCloud, DebugGeometry};

pub enum ObjClusterWriteConfig {
    Points { point_size: f32 },
    HashGridCells,
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

    /// Creates a new `Cluster` with the given `index_start` and `len`.
    pub fn new(index_start: u32, len: u32) -> Self {
        Self { index_start, len }
    }

    /// Returns the index into the `Page`'s `point_positions` and `point_colors` where the `Cluster` starts.
    pub fn index_start(&self) -> u32 {
        self.index_start
    }

    /// Returns the number of points in the `Cluster`.
    pub fn len(&self) -> u32 {
        self.len
    }
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

    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the positions of the points in the `Page`.
    pub fn point_positions(&self) -> &[Vector3<f32>] {
        &self.point_positions
    }

    /// Returns the colors of the points in the `Page`.
    pub fn point_colors(&self) -> &[ByteColor3] {
        &self.point_colors
    }

    /// Returns the `Cluster`s of the `Page`.
    pub fn clusters(&self) -> &[Cluster] {
        &self.clusters
    }

    /// Pushes a new `Cluster` to the `Page`.
    pub fn push<'p, 'c>(
        &mut self,
        point_positions: impl Iterator<Item = &'p Vector3<f32>> + Clone,
        point_colors: impl Iterator<Item = &'c ByteColor3> + Clone,
    ) {
        jeriya_shared::assert_eq! {
            point_positions.clone().into_iter().count(), point_colors.clone().into_iter().count(),
            "point_positions and point_colors must have the same length"
        }

        let index_start = self.point_positions.len() as u32;
        self.point_positions.extend(point_positions);
        self.point_colors.extend(point_colors);
        let len = self.point_positions.len() as u32 - index_start;
        self.clusters.push(Cluster::new(index_start, len));
    }

    /// Returns `true` if the `Page` has space for another `Cluster`.
    pub fn has_space(&self) -> bool {
        self.clusters.len() + 1 < Page::MAX_CLUSTERS
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ClusteredPointCloud {
    pages: Vec<Page>,
    debug_geometry: Option<DebugGeometry>,
}

impl ClusteredPointCloud {
    /// Creates a new `ClusteredPointCloud` from the given `SimplePointCloud`.
    pub fn from_simple_point_cloud(simple_point_cloud: &SimplePointCloud) -> Self {
        let target_points_per_cell = 256;
        let mut debug_hash_grid_cells = Vec::new();

        let mut unique_index_counter = AtomicUsize::new(0);
        let mut context = Context {
            point_positions: simple_point_cloud.point_positions(),
            unique_index_counter: &mut unique_index_counter,
            plot_directory: None,
            debug_hash_grid_cells: &mut Some(&mut debug_hash_grid_cells),
        };
        let hash_grid =
            ClusterHashGrid::with_debug_options(Selection::All, target_points_per_cell, BoundingBoxStrategy::Auto, &mut context);

        // Creates a priority queue that is used to process the cells starting from the lowest levels.
        let mut priority_queue = create_priority_queue(&hash_grid);
        jeriya_shared::assert!(priority_queue.iter().map(|cell| cell.unique_index).all_unique());

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
                simple_point_cloud.point_positions(),
                simple_point_cloud.point_colors(),
            );
            processed_cells_indices.insert(pivot_cell.unique_index);

            // Sample the neighboring cells and insert them into the page
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        if x == 0 && y == 0 && z == 0 {
                            continue;
                        }
                        let neighbor_sample = Vector3::new(
                            pivot_cell.aabb.center().x + x as f32 * pivot_cell.aabb.size().x,
                            pivot_cell.aabb.center().y + y as f32 * pivot_cell.aabb.size().y,
                            pivot_cell.aabb.center().z + z as f32 * pivot_cell.aabb.size().z,
                        );
                        if let Some((unique_index, points)) = hash_grid.get_leaf(neighbor_sample) {
                            jeriya_shared::assert!(pivot_cell.unique_index != unique_index);
                            if processed_cells_indices.contains(&unique_index) {
                                continue;
                            }
                            insert_into_page(
                                &mut pages,
                                points,
                                simple_point_cloud.point_positions(),
                                simple_point_cloud.point_colors(),
                            );
                            processed_cells_indices.insert(unique_index);
                        }
                    }
                }
            }
        }

        info!("Number of hash grid cells for debugging: {}", debug_hash_grid_cells.len());

        let debug_geometry = DebugGeometry {
            hash_grid_cells: debug_hash_grid_cells,
        };
        Self {
            pages,
            debug_geometry: Some(debug_geometry),
        }
    }

    /// Returns the `Page`s of the `ClusteredPointCloud`.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Returns the `DebugGeometry` of the `ClusteredPointCloud`.
    pub fn debug_geometry(&self) -> Option<&DebugGeometry> {
        self.debug_geometry.as_ref()
    }

    pub fn to_obj(
        &self,
        mut obj_writer: impl Write,
        mut mtl_writer: impl Write,
        mtl_filename: &str,
        config: &ObjClusterWriteConfig,
    ) -> io::Result<()> {
        match config {
            ObjClusterWriteConfig::Points { point_size } => {
                writeln!(obj_writer, "mtllib {mtl_filename}")?;

                // Write OBJ file
                let mut vertex_index = 1;
                let mut global_cluster_index = 0;
                for (page_index, page) in self.pages().iter().enumerate() {
                    for (cluster_index, cluster) in page.clusters().iter().enumerate() {
                        writeln!(obj_writer, "")?;
                        writeln!(obj_writer, "# Cluster {cluster_index} in page {page_index}")?;
                        writeln!(obj_writer, "o cluster_{global_cluster_index}")?;
                        writeln!(obj_writer, "usemtl cluster_{global_cluster_index}")?;
                        for index in cluster.index_start()..cluster.index_start() + cluster.len() {
                            let position = &page.point_positions()[index as usize];
                            let (a, b, c) = SimplePointCloud::create_triangle_for_point(position, *point_size)?;
                            writeln!(obj_writer, "v {} {} {}", a.x, a.y, a.z)?;
                            writeln!(obj_writer, "v {} {} {}", b.x, b.y, b.z)?;
                            writeln!(obj_writer, "v {} {} {}", c.x, c.y, c.z)?;
                        }
                        for i in 0..cluster.len() {
                            let f0 = vertex_index + i * 3;
                            let f1 = vertex_index + i * 3 + 1;
                            let f2 = vertex_index + i * 3 + 2;
                            writeln!(obj_writer, "f {f0} {f1} {f2}")?;
                        }
                        vertex_index += cluster.len() * 3;
                        global_cluster_index += 1;
                    }
                }

                // Write MTL file
                let mut global_clutser_index = 0;
                for (page_index, page) in self.pages().iter().enumerate() {
                    for (cluster_index, _cluster) in page.clusters().iter().enumerate() {
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
            ObjClusterWriteConfig::HashGridCells => {
                let mut vertex_count = 0;
                if let Some(debug_geometry) = &self.debug_geometry() {
                    for (aabb_index, aabb) in debug_geometry.hash_grid_cells.iter().enumerate() {
                        writeln!(obj_writer, "# Bounding box of the grid cell {aabb_index}")?;
                        vertex_count += write_bounding_box_o(&format!("hash_grid_cell_{aabb_index}"), vertex_count, &mut obj_writer, aabb)?;
                        writeln!(obj_writer, "")?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::fmt::Debug for ClusteredPointCloud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClusteredPointCloud").field("pages", &self.pages.len()).finish()
    }
}

struct LeafCell {
    unique_index: usize,
    aabb: AABB,
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
                    aabb: cell_content.aabb,
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
    let has_space = pages.last().map_or(false, |page| page.has_space());
    if !has_space {
        pages.push(Page::default());
    }

    // Insert the cluster into the page
    let Some(last_page) = pages.last_mut() else {
        panic!("Failed to get the last page");
    };
    last_page.push(
        points.iter().map(|&index| &point_positions[index]),
        points.iter().map(|&index| &point_colors[index]),
    );
}
