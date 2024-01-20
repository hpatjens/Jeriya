use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    time::Instant,
};

use jeriya_shared::{
    aabb::AABB,
    byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt},
    colors_transform::Hsl,
    itertools::Itertools,
    log::{info, trace},
    nalgebra::Vector3,
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

use crate::point_cloud::point_clustering_octree::ProtoCluster;

use super::{
    point_clustering_octree::{BuildContext, PointClusteringOctree},
    simple_point_cloud::SimplePointCloud,
};

pub enum ObjClusterWriteConfig {
    Points { point_size: f32, depth: usize },
}

/// Index of the `Cluster` in the `ClusteredPointCloud`.
#[repr(C)]
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterIndex {
    /// Index into the `Page`s
    pub page_index: usize,
    /// Index into the `Cluster`s of the `Page`
    pub cluster_index: usize,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
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
            point_positions.clone().count() <= Cluster::MAX_POINTS,
            "The cluster has too many point positions"
        );
        jeriya_shared::assert!(
            point_colors.clone().count() <= Cluster::MAX_POINTS,
            "The cluster has too many point colors"
        );
        jeriya_shared::assert_eq! {
            point_positions.clone().count(), point_colors.clone().count(),
            "point_positions and point_colors must have the same length"
        }

        let index_start = self.point_positions.len() as u32;
        self.point_positions.extend(point_positions.clone());
        self.point_colors.extend(point_colors);
        let len = self.point_positions.len() as u32 - index_start;
        let aabb = AABB::from_ref_iter(point_positions.clone());
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

#[derive(Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusteredPointCloud {
    root_cluster_index: ClusterIndex,
    pages: Vec<Page>,
    max_cluster_depth: usize,
}

impl ClusteredPointCloud {
    pub fn from_simple_point_cloud(simple_point_cloud: &SimplePointCloud) -> Self {
        let start = Instant::now();

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
        visit(octree.root(), 0, &mut pages, simple_point_cloud);

        let root_cluster_index = ClusterIndex {
            page_index: pages.len() - 1,
            cluster_index: pages.last().expect("failed to get last page").clusters.len() - 1,
        };
        trace!("Root cluster index: {:?}", root_cluster_index);

        info!("Computing the clusters took {} ms", start.elapsed().as_secs_f32());

        Self {
            root_cluster_index,
            pages,
            max_cluster_depth: octree.max_proto_cluster_depth(),
        }
    }

    /// Returns the `Page`s of the `ClusteredPointCloud`.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Returns the index of the root `Cluster` in the `ClusteredPointCloud`.
    pub fn root_cluster_index(&self) -> ClusterIndex {
        self.root_cluster_index.clone()
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
        let mut data = data.iter().map(|(a, b)| (*a, *b)).collect::<Vec<_>>();
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
            data.entry(page.point_positions().len())
                .and_modify(|count| *count += 1)
                .or_insert(1usize);
        }

        Self::plot_histogram(self, &data, filepath, "Page fill level histogram")
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
                        writeln!(obj_writer)?;
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
                        writeln!(mtl_writer)?;
                        global_clutser_index += 1;
                    }
                }

                Ok(())
            }
        }
    }

    /// Writes the point cloud as an OBJ file. The MTL file is written to the same directory.
    pub fn to_obj_file(&self, config: &ObjClusterWriteConfig, filepath: &impl AsRef<Path>) -> io::Result<()> {
        let obj_filepath = filepath.as_ref().with_extension("obj");
        let mtl_filepath = filepath.as_ref().with_extension("mtl");
        let obj_file = File::create(obj_filepath)?;
        let mtl_file = File::create(&mtl_filepath)?;
        let mtl_filename = mtl_filepath
            .file_name()
            .expect("Failed to get MTL filename")
            .to_str()
            .expect("Failed to convert MTL filename to str");
        self.to_obj(obj_file, mtl_file, mtl_filename, config)
    }

    /// Serializes the `Page` table into a stream.
    fn serialize_page_table_into<W: Write + Seek>(&self, mut writer: W, page_table: &HashMap<u64, u64>) -> io::Result<()> {
        writer.write_u64::<LittleEndian>(self.pages.len() as u64)?;
        for page_index in 0..self.pages.len() {
            let page_index = page_index as u64;
            let page_offset = *page_table.get(&page_index).expect("Failed to get page offset");
            writer.write_u64::<LittleEndian>(page_index)?;
            writer.write_u64::<LittleEndian>(page_offset)?;
        }
        Ok(())
    }

    /// Serializes the header of the `PointCloud` into a stream.
    fn serialize_header_into<W: Write + Seek>(&self, mut writer: W, pages_offset: u64, page_table_offset: u64) -> io::Result<()> {
        writer.write_u64::<LittleEndian>(self.root_cluster_index.page_index as u64)?;
        writer.write_u64::<LittleEndian>(self.root_cluster_index.cluster_index as u64)?;
        writer.write_u64::<LittleEndian>(pages_offset)?;
        writer.write_u64::<LittleEndian>(page_table_offset)?;
        Ok(())
    }

    /// Serializes the `Page`s into a stream.
    fn serlialize_pages_into<W: Write + Seek>(&self, mut writer: W) -> io::Result<HashMap<u64, u64>> {
        let mut page_table = HashMap::<u64, u64>::new();
        for (page_index, page) in self.pages.iter().enumerate() {
            // Insert the offset into the page table
            let page_start = writer.stream_position()?;
            page_table.insert(page_index as u64, page_start);

            let len = page.point_positions().len() as u64;
            writer.write_u64::<LittleEndian>(len)?;

            // Write the point positions
            let positions_ptr = page.point_positions.as_ptr() as *const u8;
            let positions_size = page.point_positions.len() * std::mem::size_of::<Vector3<f32>>();
            let positions: &[u8] = unsafe { std::slice::from_raw_parts(positions_ptr, positions_size) };
            let _ = writer.write(positions)?;

            // Write the point colors
            let colors_ptr = page.point_colors.as_ptr() as *const u8;
            let colors_size = page.point_colors.len() * std::mem::size_of::<ByteColor3>();
            let colors: &[u8] = unsafe { std::slice::from_raw_parts(colors_ptr, colors_size) };
            let _ = writer.write(colors)?;

            // Write the page
            writer.write_u64::<LittleEndian>(page.clusters.len() as u64)?;
            for cluster in &page.clusters {
                writer.write_u64::<LittleEndian>(cluster.index_start as u64)?;
                writer.write_u64::<LittleEndian>(cluster.len as u64)?;
                writer.write_f32::<LittleEndian>(cluster.aabb.min.x)?;
                writer.write_f32::<LittleEndian>(cluster.aabb.min.y)?;
                writer.write_f32::<LittleEndian>(cluster.aabb.min.z)?;
                writer.write_f32::<LittleEndian>(cluster.aabb.max.x)?;
                writer.write_f32::<LittleEndian>(cluster.aabb.max.y)?;
                writer.write_f32::<LittleEndian>(cluster.aabb.max.z)?;
                writer.write_f32::<LittleEndian>(cluster.center.x)?;
                writer.write_f32::<LittleEndian>(cluster.center.y)?;
                writer.write_f32::<LittleEndian>(cluster.center.z)?;
                writer.write_f32::<LittleEndian>(cluster.radius)?;
                writer.write_u64::<LittleEndian>(cluster.depth as u64)?;
                writer.write_u64::<LittleEndian>(cluster.level as u64)?;
                writer.write_u64::<LittleEndian>(cluster.children.len() as u64)?;
                for child in &cluster.children {
                    writer.write_u64::<LittleEndian>(child.page_index as u64)?;
                    writer.write_u64::<LittleEndian>(child.cluster_index as u64)?;
                }
            }
        }
        Ok(page_table)
    }

    /// Serializes the `PointCloud` into a stream.
    pub fn serialize_into<W: Write + Seek>(&self, mut writer: W) -> io::Result<()> {
        // Leave space for the header
        let header_size = 4 * std::mem::size_of::<u64>();
        writer.seek(SeekFrom::Start(header_size as u64))?;

        // Write the pages
        let pages_offset = writer.stream_position()?;
        let page_table = self.serlialize_pages_into(&mut writer)?;

        // Write the page table
        let page_table_offset = writer.stream_position()?;
        self.serialize_page_table_into(&mut writer, &page_table)?;

        // Write header at the start of the stream
        writer.rewind()?;
        self.serialize_header_into(&mut writer, pages_offset, page_table_offset)?;

        Ok(())
    }

    /// Serializes the `PointCloud` to a file.
    pub fn serialize_to_file(&self, filepath: &impl AsRef<Path>) -> crate::Result<()> {
        let mut file = File::create(filepath)?;
        let start = Instant::now();
        self.serialize_into(&mut file)?;
        info!("Serializing the point cloud took {} ms", start.elapsed().as_secs_f32());
        Ok(())
    }

    /// Deserializes the `Page` table from a stream. A `Vec` is returned instead of
    /// a `HashMap` because it also gives the order of the pages. It can be converted
    /// into a `HashMap` with `Vec::into_iter().collect::<HashMap<_, _>>()`. The first
    /// element of the tuple is the page index and the second element is the page offset.
    fn deserialize_page_table_from<R: Read + Seek>(mut reader: R) -> io::Result<Vec<(u64, u64)>> {
        let page_table_len = reader.read_u64::<LittleEndian>()?;
        let mut page_table = Vec::with_capacity(page_table_len as usize);
        for _ in 0..page_table_len {
            let page_index = reader.read_u64::<LittleEndian>()?;
            let page_offset = reader.read_u64::<LittleEndian>()?;
            page_table.push((page_index, page_offset));
        }
        Ok(page_table)
    }

    /// Deserializes the header of the `PointCloud` from a stream.
    fn deserialize_header_from<R: Read + Seek>(mut reader: R) -> io::Result<(u64, u64, u64, u64)> {
        let root_cluster_page_index = reader.read_u64::<LittleEndian>()?;
        let root_cluster_index = reader.read_u64::<LittleEndian>()?;
        let pages_offset = reader.read_u64::<LittleEndian>()?;
        let page_table_offset = reader.read_u64::<LittleEndian>()?;
        Ok((root_cluster_page_index, root_cluster_index, pages_offset, page_table_offset))
    }

    /// Deserializes the `Page`s from a stream.
    fn deserialize_pages_from<R: Read + Seek>(mut reader: R, page_offsets: &Vec<(u64, u64)>) -> io::Result<Vec<Page>> {
        let mut pages = Vec::<Page>::new();
        for (_page_index, page_offset) in page_offsets {
            reader.seek(SeekFrom::Start(*page_offset))?;

            let len = reader.read_u64::<LittleEndian>()? as usize;

            // Read the point positions
            let mut point_positions = Vec::<Vector3<f32>>::with_capacity(len);
            let positions_size = len * std::mem::size_of::<Vector3<f32>>();
            let mut positions = vec![0u8; positions_size];
            reader.read_exact(&mut positions)?;
            let positions_ptr = positions.as_ptr() as *const Vector3<f32>;
            let positions: &[Vector3<f32>] = unsafe { std::slice::from_raw_parts(positions_ptr, len) };
            point_positions.extend_from_slice(positions);

            // Read the point colors
            let mut point_colors = Vec::<ByteColor3>::with_capacity(len);
            let colors_size = len * std::mem::size_of::<ByteColor3>();
            let mut colors = vec![0u8; colors_size];
            reader.read_exact(&mut colors)?;
            let colors_ptr = colors.as_ptr() as *const ByteColor3;
            let colors: &[ByteColor3] = unsafe { std::slice::from_raw_parts(colors_ptr, len) };
            point_colors.extend_from_slice(colors);

            // Read the clusters
            let clusters_len = reader.read_u64::<LittleEndian>()? as usize;
            let mut clusters = Vec::<Cluster>::with_capacity(clusters_len);
            for _ in 0..clusters_len {
                let index_start = reader.read_u64::<LittleEndian>()? as u32;
                let len = reader.read_u64::<LittleEndian>()? as u32;
                let min_x = reader.read_f32::<LittleEndian>()?;
                let min_y = reader.read_f32::<LittleEndian>()?;
                let min_z = reader.read_f32::<LittleEndian>()?;
                let max_x = reader.read_f32::<LittleEndian>()?;
                let max_y = reader.read_f32::<LittleEndian>()?;
                let max_z = reader.read_f32::<LittleEndian>()?;
                let center_x = reader.read_f32::<LittleEndian>()?;
                let center_y = reader.read_f32::<LittleEndian>()?;
                let center_z = reader.read_f32::<LittleEndian>()?;
                let radius = reader.read_f32::<LittleEndian>()?;
                let depth = reader.read_u64::<LittleEndian>()? as usize;
                let level = reader.read_u64::<LittleEndian>()? as usize;
                let children_len = reader.read_u64::<LittleEndian>()? as usize;
                let mut children = Vec::<ClusterIndex>::with_capacity(children_len);
                for _ in 0..children_len {
                    let page_index = reader.read_u64::<LittleEndian>()? as usize;
                    let cluster_index = reader.read_u64::<LittleEndian>()? as usize;
                    children.push(ClusterIndex { page_index, cluster_index });
                }
                clusters.push(Cluster {
                    index_start,
                    len,
                    aabb: AABB::new(Vector3::new(min_x, min_y, min_z), Vector3::new(max_x, max_y, max_z)),
                    center: Vector3::new(center_x, center_y, center_z),
                    radius,
                    depth,
                    level,
                    children,
                });
            }

            pages.push(Page {
                point_positions,
                point_colors,
                clusters,
            });
        }
        Ok(pages)
    }

    /// Deserializes the `PointCloud` from a stream.
    pub fn deserialize_from<R: Read + Seek>(mut reader: R) -> io::Result<Self> {
        // Read the header
        let (root_cluster_page_index, root_cluster_index, pages_offset, page_table_offset) = Self::deserialize_header_from(&mut reader)?;

        // Read the page table
        reader.seek(SeekFrom::Start(page_table_offset))?;
        let page_offsets = Self::deserialize_page_table_from(&mut reader)?;

        // Read the pages
        reader.seek(SeekFrom::Start(pages_offset))?;
        let pages = Self::deserialize_pages_from(&mut reader, &page_offsets)?;

        let root_cluster_index = ClusterIndex {
            page_index: root_cluster_page_index as usize,
            cluster_index: root_cluster_index as usize,
        };
        let max_cluster_depth = pages
            .iter()
            .map(|page| page.clusters.iter().map(|cluster| cluster.depth).max().unwrap_or(0))
            .max()
            .unwrap_or(0);

        Ok(Self {
            root_cluster_index,
            pages,
            max_cluster_depth,
        })
    }

    /// Deserializes the `PointCloud` from a file.
    pub fn deserialize_from_file(filepath: &impl AsRef<Path>) -> crate::Result<Self> {
        let mut file = File::open(filepath)?;
        let start = Instant::now();
        let result = Self::deserialize_from(&mut file)?;
        info!("Deserializing the point cloud took {} ms", start.elapsed().as_secs_f32());
        Ok(result)
    }

    /// Deserializes the `Page` table from a file.
    pub fn deserialize_page_table_from_file(filepath: &impl AsRef<Path>) -> crate::Result<HashMap<u64, u64>> {
        let mut file = File::open(filepath).expect("Failed to open file");
        let (_, _, _, page_table_offset) = Self::deserialize_header_from(&mut file)?;
        file.seek(SeekFrom::Start(page_table_offset)).expect("Failed to seek to page table");
        let page_table = Self::deserialize_page_table_from(&mut file)?;
        Ok(page_table.into_iter().collect::<HashMap<_, _>>())
    }
}

impl std::fmt::Debug for ClusteredPointCloud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClusteredPointCloud").field("pages", &self.pages.len()).finish()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use jeriya_shared::function_name;
    use jeriya_test::create_test_result_folder_for_function;

    use crate::model::ModelAsset;

    use super::*;

    #[test]
    fn test_sample_from_model() {
        env_logger::builder().filter_level(jeriya_shared::log::LevelFilter::Trace).init();

        let directory = create_test_result_folder_for_function(function_name!());

        let model = ModelAsset::import("../sample_assets/models/suzanne.glb").unwrap();
        let simple_point_cloud = SimplePointCloud::sample_from_model(&model, 200.0, 1.0);
        let clustered_point_cloud = ClusteredPointCloud::from_simple_point_cloud(&simple_point_cloud);

        for depth in 0..=clustered_point_cloud.max_cluster_depth() {
            let config = ObjClusterWriteConfig::Points { point_size: 0.02, depth };
            clustered_point_cloud
                .to_obj_file(&config, &directory.join(format!("point_cloud_depth{depth}.obj")))
                .unwrap();
        }

        clustered_point_cloud
            .plot_cluster_fill_level_histogram(&directory.join("cluster_fill_level_histogram.svg"))
            .unwrap();

        clustered_point_cloud
            .plot_page_fill_level_histogram(&directory.join("page_fill_level_histogram.svg"))
            .unwrap();

        clustered_point_cloud.write_statisics(&directory.join("statistics.json")).unwrap();
    }

    #[test]
    fn serialize_and_deserialize() {
        let simple_point_cloud =
            SimplePointCloud::sample_from_model(&ModelAsset::import("../sample_assets/models/suzanne.glb").unwrap(), 200.0, 1.0);
        let clustered_point_cloud = ClusteredPointCloud::from_simple_point_cloud(&simple_point_cloud);
        let mut file = Cursor::new(Vec::new());
        clustered_point_cloud.serialize_into(&mut file).unwrap();
        file.rewind().unwrap();
        let result = ClusteredPointCloud::deserialize_from(file).unwrap();
        assert_eq!(clustered_point_cloud, result);
    }

    #[test]
    fn deserialize_page_table_from_file_smoke() {
        let simple_point_cloud =
            SimplePointCloud::sample_from_model(&ModelAsset::import("../sample_assets/models/suzanne.glb").unwrap(), 200.0, 1.0);
        let clustered_point_cloud = ClusteredPointCloud::from_simple_point_cloud(&simple_point_cloud);
        let folder = create_test_result_folder_for_function(function_name!());
        let filepath = folder.join("point_cloud.bin");
        clustered_point_cloud.serialize_to_file(&filepath).unwrap();
        let result = ClusteredPointCloud::deserialize_page_table_from_file(&filepath).unwrap();
        assert_eq!(clustered_point_cloud.pages.len(), result.len());
    }
}
