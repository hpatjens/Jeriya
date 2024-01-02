use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
    path::Path,
    sync::atomic::AtomicUsize,
    time::Instant,
};

use jeriya_shared::{
    aabb::AABB,
    colors_transform::Hsl,
    itertools::Itertools,
    log::{info, trace},
    nalgebra::Vector3,
    obj_writer::write_bounding_box_o,
    plotters::{
        backend::{DrawingBackend, SVGBackend},
        chart::ChartBuilder,
        coord::ranged1d::IntoSegmentedCoord,
        drawing::{DrawingAreaErrorKind, IntoDrawingArea},
        series::Histogram,
        style::Color,
        style::{BLUE, WHITE},
    },
    rand, serde_json, ByteColor3,
};
use serde::{Deserialize, Serialize};

use crate::point_cloud::{
    cluster_graph::ClusterGraph,
    point_clustering_hash_grid::{CellContent, CellType, ClusterHashGrid, Context},
    point_clustering_octree::ProtoCluster,
};

use super::{
    point_clustering_octree::{BuildContext, PointClusteringOctree},
    simple_point_cloud::SimplePointCloud,
    DebugGeometry,
};

pub enum ObjClusterWriteConfig {
    Points { point_size: f32, depth: usize },
    HashGridCells,
}

/// Index of the `Cluster` in the `ClusteredPointCloud`.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ClusterIndex {
    /// Index into the `Page`s
    pub page_index: usize,
    /// Index into the `Cluster`s of the `Page`
    pub cluster_index: usize,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// Index into the `Page`'s `point_positions` and `point_colors` `Vec`s
    pub index_start: u32,
    /// Number of points in the cluster
    pub len: u32,
    /// The bounding box of the cluster
    pub aabb: AABB,
    /// The center of the cluster
    pub center: Vector3<f32>,
    /// The radius of the cluster
    pub radius: f32,
    /// The depth of the cluster in the tree. The root node has a depth of 0.
    pub depth: usize,
    /// The level of the cluster in the tree. The leaf node has a level of 0.
    pub level: usize,
    /// The children of the cluster
    pub children: Vec<ClusterIndex>,
}

impl Cluster {
    /// The maximum number of points a cluster can have.
    pub const MAX_POINTS: usize = 256;
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

    /// Pushes a new `Cluster` to the `Page` and return the index of the cluster in the `Page`.
    ///
    /// # Panics
    ///
    /// * If the `Page` is full. This can be checked with [`ClusteredPointCloud::has_space`].
    /// * If the `point_positions` and `point_colors` `Iterator`s have different lengths.
    pub fn push<'p, 'c>(
        &mut self,
        point_positions: impl Iterator<Item = &'p Vector3<f32>> + Clone,
        point_colors: impl Iterator<Item = &'c ByteColor3> + Clone,
        depth: usize,
        level: usize,
        children: Vec<ClusterIndex>,
    ) -> usize {
        jeriya_shared::assert!(self.has_space(), "The page is full");
        jeriya_shared::assert!(children.len() <= 2, "too many children");
        jeriya_shared::assert!(
            point_positions.clone().into_iter().count() <= Cluster::MAX_POINTS,
            "The cluster has too many point positions"
        );
        jeriya_shared::assert!(
            point_colors.clone().into_iter().count() <= Cluster::MAX_POINTS,
            "The cluster has too many point colors"
        );
        jeriya_shared::assert_eq! {
            point_positions.clone().into_iter().count(), point_colors.clone().into_iter().count(),
            "point_positions and point_colors must have the same length"
        }

        let index_start = self.point_positions.len() as u32;
        self.point_positions.extend(point_positions.clone());
        self.point_colors.extend(point_colors);
        let len = self.point_positions.len() as u32 - index_start;
        let aabb = AABB::from_iter(point_positions.clone());
        let center = point_positions.clone().fold(Vector3::zeros(), |acc, position| acc + position) / len as f32;
        let radius = point_positions.fold(0.0, |acc, position| {
            let distance = (position - center).norm();
            if distance > acc {
                distance
            } else {
                acc
            }
        });

        let index = self.clusters.len();
        self.clusters.push(Cluster {
            index_start,
            len,
            aabb,
            center,
            radius,
            depth,
            level,
            children,
        });
        index
    }

    /// Returns `true` if the `Page` has space for another `Cluster`.
    pub fn has_space(&self) -> bool {
        let result = self.clusters.len() + 1 <= Page::MAX_CLUSTERS;
        if result {
            jeriya_shared::assert!(self.point_positions.len() + Cluster::MAX_POINTS <= Page::MAX_POINTS);
            jeriya_shared::assert!(self.point_colors.len() + Cluster::MAX_POINTS <= Page::MAX_POINTS);
        }
        result
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ClusteredPointCloud {
    root_cluster_index: ClusterIndex,
    pages: Vec<Page>,
    max_cluster_depth: usize,
    debug_geometry: Option<DebugGeometry>,
}

impl ClusteredPointCloud {
    /// Creates a new `ClusteredPointCloud` from the given `SimplePointCloud`.
    #[deprecated(note = "please use `from_simple_point_cloud` instead")]
    pub fn from_simple_point_cloud_flat(simple_point_cloud: &SimplePointCloud) -> Self {
        let start = Instant::now();

        let target_points_per_cell = Cluster::MAX_POINTS;
        let mut debug_hash_grid_cells = Vec::new();

        let hash_grid = ClusterHashGrid::from_all(
            target_points_per_cell,
            &mut Context {
                point_positions: simple_point_cloud.point_positions(),
                unique_index_counter: &mut AtomicUsize::new(0),
                plot_directory: None,
                debug_hash_grid_cells: Some(&mut debug_hash_grid_cells),
            },
        );

        // Creates a priority queue that is used to process the cells starting from the lowest levels.
        let mut priority_queue = create_priority_queue(&hash_grid);
        jeriya_shared::assert!(
            priority_queue.iter().all(|cell| cell.indices.len() <= Cluster::MAX_POINTS),
            "A cell has too many points"
        );
        jeriya_shared::assert!(
            priority_queue.iter().map(|cell| cell.unique_index).all_unique(),
            "The priority queue contains duplicate cells"
        );

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
                &pivot_cell.indices,
                simple_point_cloud.point_positions(),
                simple_point_cloud.point_colors(),
                0,
                0,
                Vec::new(),
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
                                0,
                                0,
                                Vec::new(),
                            );
                            processed_cells_indices.insert(unique_index);
                        }
                    }
                }
            }
        }

        info!("Number of hash grid cells for debugging: {}", debug_hash_grid_cells.len());
        info!("Computing the clusters took {} ms", start.elapsed().as_secs_f32());

        let debug_geometry = DebugGeometry {
            hash_grid_cells: debug_hash_grid_cells,
        };
        Self {
            root_cluster_index: ClusterIndex {
                page_index: 0,
                cluster_index: 0,
            },
            pages,
            max_cluster_depth: 0,
            debug_geometry: Some(debug_geometry),
        }
    }

    /// Creates a new `ClusteredPointCloud` from the given `SimplePointCloud`.
    #[deprecated(note = "please use `from_simple_point_cloud` instead")]
    pub fn from_simple_point_cloud_cluster_graph(simple_point_cloud: &SimplePointCloud, debug_directory: Option<&Path>) -> Self {
        let start = Instant::now();

        let target_points_per_cell = Cluster::MAX_POINTS;
        let mut debug_hash_grid_cells = Vec::new();

        let hash_grid = ClusterHashGrid::from_all(
            target_points_per_cell,
            &mut Context {
                point_positions: simple_point_cloud.point_positions(),
                unique_index_counter: &mut AtomicUsize::new(0),
                plot_directory: debug_directory.map(Path::to_path_buf),
                debug_hash_grid_cells: Some(&mut debug_hash_grid_cells),
            },
        );

        // Creates a priority queue that is used to process the cells starting from the lowest levels.
        let mut priority_queue = create_priority_queue(&hash_grid);
        jeriya_shared::assert!(
            priority_queue.iter().all(|cell| cell.indices.len() <= Cluster::MAX_POINTS),
            "A cell has too many points"
        );
        jeriya_shared::assert!(
            priority_queue.iter().map(|cell| cell.unique_index).all_unique(),
            "The priority queue contains duplicate cells"
        );

        // The cells are processed from the lowest levels to the highest levels. When a cell is
        // processed, it is inserted into this HashSet so that larger cells in it's proximity
        // don't identify this cell as a neighbor. This is likely because for larger cells, the
        // step size for finding the neighbor might be so large that directly neighboring cells
        // are skipped.
        let mut processed_cells_indices = HashSet::new();

        let mut cluster_graph = ClusterGraph::new();
        for leaf in &priority_queue {
            let mut neighboring_cells = Vec::new();
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        if x == 0 && y == 0 && z == 0 {
                            continue;
                        }
                        let neighbor_sample = Vector3::new(
                            leaf.aabb.center().x + x as f32 * leaf.aabb.size().x,
                            leaf.aabb.center().y + y as f32 * leaf.aabb.size().y,
                            leaf.aabb.center().z + z as f32 * leaf.aabb.size().z,
                        );
                        if let Some((unique_index, _)) = hash_grid.get_leaf(neighbor_sample) {
                            jeriya_shared::assert!(leaf.unique_index != unique_index);
                            if processed_cells_indices.contains(&unique_index) {
                                // We don't want to consider smaller cells that are not direct
                                // neigbors as neighbors.
                                continue;
                            }
                            if !neighboring_cells.contains(&unique_index) {
                                neighboring_cells.push(unique_index);
                            }
                        }
                    }
                }
            }
            processed_cells_indices.insert(leaf.unique_index);
            cluster_graph.push_cluster(leaf.unique_index, leaf.indices.clone(), neighboring_cells);
        }

        // Smaller nodes know their neighbors but larger nodes don't know smaller nodes when
        // they are their neighbors. Therefore, we have to create bidirectional connections.
        cluster_graph.create_bidirectional_connections();

        jeriya_shared::assert!(cluster_graph.validate_bidirectional(), "failed to validate ClusterGraph");
        if let Some(debug_directory) = debug_directory {
            let dot = cluster_graph.to_dot();
            std::fs::write(debug_directory.join("cluster_graph.dot"), dot).expect("failed to write cluster graph to file");
        }

        // The information from the cells is collected into the last page and when it is full,
        // a new page is created.
        let mut pages = vec![Page::default()];
        info!("Number of pages: {}", pages.len());
        info!("Number of hash grid cells for debugging: {}", debug_hash_grid_cells.len());
        info!("Computing the clusters took {} ms", start.elapsed().as_secs_f32());

        let debug_geometry = DebugGeometry {
            hash_grid_cells: debug_hash_grid_cells,
        };
        Self {
            root_cluster_index: ClusterIndex {
                page_index: 0,
                cluster_index: 0,
            },
            pages,
            max_cluster_depth: 0,
            debug_geometry: Some(debug_geometry),
        }
    }

    pub fn from_simple_point_cloud(simple_point_cloud: &SimplePointCloud, debug_directory: Option<&Path>) -> Self {
        let start = Instant::now();

        let target_points_per_cell = Cluster::MAX_POINTS;
        let mut debug_hash_grid_cells = Vec::new();

        let build_parameters = &BuildContext {
            cluster_point_count: Cluster::MAX_POINTS,
            point_positions: simple_point_cloud.point_positions(),
        };
        let octree = PointClusteringOctree::new(build_parameters);

        let mut pages = vec![Page::default()];

        // Packs the proto clusters into pages and returns the (page, cluster) indices of the packed cluster.
        fn visit(proto_cluster: &ProtoCluster, depth: usize, pages: &mut Vec<Page>, simple_point_cloud: &SimplePointCloud) -> ClusterIndex {
            // Pack the children into pages and collect the (page, cluster) indices of the packed clusters.
            // The children have to be packed first, so that the indices of the children are known.
            let children = proto_cluster
                .children
                .iter()
                .map(|child| visit(child, depth + 1, pages, simple_point_cloud))
                .collect_vec();

            // Either take the last page or create a new one if the last page is full.
            let has_space = pages.last().map(|page| page.has_space()).unwrap_or(false);
            if !has_space {
                pages.push(Page::new());
            }
            let page_index = pages.len() - 1;
            let page = pages.last_mut().expect("failed to get last page");

            // Create and insert a new cluster into the page.
            let positions = simple_point_cloud.point_positions();
            let colors = simple_point_cloud.point_colors();
            let point_positions = proto_cluster.indices.iter().map(|index| &positions[*index]);
            let point_colors = proto_cluster.indices.iter().map(|index| &colors[*index]);

            trace!("Pushing cluster with {} points", proto_cluster.indices.len());

            let cluster_index = page.push(point_positions, point_colors, depth, proto_cluster.level, children);

            ClusterIndex { page_index, cluster_index }
        }
        visit(octree.root(), 0, &mut pages, &simple_point_cloud);

        // The clusters are collected from the octree into pages by starting the
        // traversal at the root. Therefore, the root cluster index is (0, 0).
        let root_cluster_index = ClusterIndex {
            page_index: 0,
            cluster_index: 0,
        };

        let debug_geometry = DebugGeometry {
            hash_grid_cells: debug_hash_grid_cells,
        };

        info!("Computing the clusters took {} ms", start.elapsed().as_secs_f32());

        Self {
            root_cluster_index,
            pages,
            max_cluster_depth: octree.max_proto_cluster_depth(),
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

    /// Returns the maximum depth of the clusters in the `ClusteredPointCloud`.
    pub fn max_cluster_depth(&self) -> usize {
        self.max_cluster_depth
    }

    pub fn write_statisics(&self, filepath: &impl AsRef<Path>) -> io::Result<()> {
        let cluster_count_at_depth = (0..=self.max_cluster_depth)
            .map(|depth| {
                self.pages
                    .iter()
                    .map(|page| page.clusters.iter().filter(|cluster| cluster.depth == depth).count())
                    .sum::<usize>()
            })
            .collect::<Vec<_>>();

        #[derive(Serialize, Deserialize)]
        struct Statistics {
            pages: usize,
            clusters: usize,
            points: usize,
            cluster_count_at_depth: Vec<usize>,
        }

        let mut file = std::fs::File::create(filepath)?;
        serde_json::to_writer_pretty(
            &mut file,
            &Statistics {
                pages: self.pages.len(),
                clusters: self.pages.iter().map(|page| page.clusters.len()).sum(),
                points: self
                    .pages
                    .iter()
                    .map(|page| page.clusters.iter().map(|cluster| cluster.len as usize).sum::<usize>())
                    .sum(),
                cluster_count_at_depth,
            },
        )?;
        Ok(())
    }

    fn plot_histogram<'a>(
        &self,
        data: &HashMap<usize, usize>,
        filepath: &impl AsRef<Path>,
        title: &str,
    ) -> Result<(), DrawingAreaErrorKind<<SVGBackend<'a> as DrawingBackend>::ErrorType>> {
        // Prepare the drawing area
        let drawing_area = SVGBackend::new(filepath, (800, 600)).into_drawing_area();
        drawing_area.fill(&WHITE)?;

        // Convert the data into the representation that plotters expects
        let mut data = data.into_iter().map(|(a, b)| (a.clone(), b.clone())).collect::<Vec<_>>();
        data.sort_by(|(a, _), (b, _)| a.cmp(b));

        let x_max = data.iter().map(|(a, _)| a).max().cloned().unwrap_or(0usize);
        let y_max = data.iter().map(|(_, b)| b).max().cloned().unwrap_or(0usize);

        let mut chart = ChartBuilder::on(&drawing_area)
            .x_label_area_size(35)
            .y_label_area_size(40)
            .margin(5)
            .caption(title, ("sans-serif", 50.0))
            .build_cartesian_2d((0usize..x_max).into_segmented(), 0usize..y_max)?;

        chart
            .configure_mesh()
            .disable_x_mesh()
            .bold_line_style(WHITE.mix(0.3))
            .y_desc("Count")
            .x_desc("Points")
            .axis_desc_style(("sans-serif", 15))
            .draw()?;

        chart.draw_series(Histogram::vertical(&chart).margin(0).style(BLUE.mix(0.5).filled()).data(data))?;

        Ok(())
    }

    /// Plots the histogram showing how many clusters have a certain number of points.
    pub fn plot_cluster_fill_level_histogram<'a>(
        &self,
        filepath: &impl AsRef<Path>,
    ) -> Result<(), DrawingAreaErrorKind<<SVGBackend<'a> as DrawingBackend>::ErrorType>> {
        // Map from the number of points in a cluster to the number of clusters with that number of points
        let mut data = HashMap::<usize, usize>::new();
        for page in &self.pages {
            for cluster in &page.clusters {
                data.entry(cluster.len as usize).and_modify(|count| *count += 1).or_insert(1usize);
            }
        }

        Self::plot_histogram(self, &data, filepath, "Cluster fill level histogram")
    }

    /// Plots the histogram showing how many pages have a certain number of points.
    pub fn plot_page_fill_level_histogram<'a>(
        &self,
        filepath: &impl AsRef<Path>,
    ) -> Result<(), DrawingAreaErrorKind<<SVGBackend<'a> as DrawingBackend>::ErrorType>> {
        // Map from the number of points in a cluster to the number of clusters with that number of points
        let mut data = HashMap::<usize, usize>::new();
        for page in &self.pages {
            data.entry(page.point_positions().len() as usize)
                .and_modify(|count| *count += 1)
                .or_insert(1usize);
        }

        Self::plot_histogram(&self, &data, filepath, "Page fill level histogram")
    }

    pub fn to_obj(
        &self,
        mut obj_writer: impl Write,
        mut mtl_writer: impl Write,
        mtl_filename: &str,
        config: &ObjClusterWriteConfig,
    ) -> io::Result<()> {
        match config {
            ObjClusterWriteConfig::Points { point_size, depth } => {
                writeln!(obj_writer, "mtllib {mtl_filename}")?;

                // Write OBJ file
                let mut vertex_index = 1;
                let mut global_cluster_index = 0;
                for (page_index, page) in self.pages().iter().enumerate() {
                    for (cluster_index, cluster) in page.clusters().iter().filter(|cluster| cluster.depth == *depth).enumerate() {
                        writeln!(obj_writer, "")?;
                        writeln!(obj_writer, "# Cluster {cluster_index} in page {page_index}")?;
                        writeln!(obj_writer, "o cluster_{global_cluster_index}")?;
                        writeln!(obj_writer, "usemtl cluster_{global_cluster_index}")?;
                        for index in cluster.index_start..cluster.index_start + cluster.len {
                            let position = &page.point_positions()[index as usize];
                            let (a, b, c) = SimplePointCloud::create_triangle_for_point(position, *point_size)?;
                            writeln!(obj_writer, "v {} {} {}", a.x, a.y, a.z)?;
                            writeln!(obj_writer, "v {} {} {}", b.x, b.y, b.z)?;
                            writeln!(obj_writer, "v {} {} {}", c.x, c.y, c.z)?;
                        }
                        for i in 0..cluster.len {
                            let f0 = vertex_index + i * 3;
                            let f1 = vertex_index + i * 3 + 1;
                            let f2 = vertex_index + i * 3 + 2;
                            writeln!(obj_writer, "f {f0} {f1} {f2}")?;
                        }
                        vertex_index += cluster.len * 3;
                        global_cluster_index += 1;
                    }
                }

                // Write MTL file
                let mut global_clutser_index = 0;
                for (page_index, page) in self.pages().iter().enumerate() {
                    for (cluster_index, _cluster) in page.clusters().iter().filter(|cluster| cluster.depth == *depth).enumerate() {
                        use jeriya_shared::colors_transform::Color;
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
    indices: Vec<usize>,
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
            CellType::Empty => {}
            CellType::Leaf(points) => {
                priority_queue.push(LeafCell {
                    unique_index: cell_content.unique_index,
                    aabb: cell_content.aabb,
                    indices: points.clone(),
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
fn insert_into_page(
    pages: &mut Vec<Page>,
    points: &[usize],
    point_positions: &[Vector3<f32>],
    point_colors: &[ByteColor3],
    depth: usize,
    level: usize,
    children: Vec<ClusterIndex>,
) {
    jeriya_shared::assert!(points.len() <= Cluster::MAX_POINTS, "The cluster has too many points");

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
        depth,
        level,
        children,
    );
}
