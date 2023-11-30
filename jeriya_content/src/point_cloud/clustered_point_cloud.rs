use jeriya_shared::{nalgebra::Vector3, ByteColor3};
use serde::{Deserialize, Serialize};

use super::DebugGeometry;

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
    /// Creates a new `ClusteredPointCloud` with the given `pages` and `debug_geometry`.
    pub fn new(pages: Vec<Page>, debug_geometry: Option<DebugGeometry>) -> Self {
        Self { pages, debug_geometry }
    }

    /// Returns the `Page`s of the `ClusteredPointCloud`.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Returns the `DebugGeometry` of the `ClusteredPointCloud`.
    pub fn debug_geometry(&self) -> Option<&DebugGeometry> {
        self.debug_geometry.as_ref()
    }
}

impl std::fmt::Debug for ClusteredPointCloud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClusteredPointCloud").field("pages", &self.pages.len()).finish()
    }
}
