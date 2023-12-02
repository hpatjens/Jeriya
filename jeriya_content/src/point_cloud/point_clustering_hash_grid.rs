use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

use jeriya_shared::{aabb::AABB, log::trace, nalgebra::Vector3};

pub type CellIndex = Vector3<i32>;

/// Stores meta information of a grid cell as well as the `CellType`.
pub struct CellContent {
    pub unique_index: usize,
    pub aabb: AABB,
    pub ty: CellType,
}

impl CellContent {
    /// Returns the number of points in the grid cell and all its children.
    pub fn len(&self) -> usize {
        match &self.ty {
            CellType::Empty => 0,
            CellType::Leaf(points) => points.len(),
            CellType::Grid(grid) => grid.cells().map(CellContent::len).sum::<usize>(),
            CellType::XAxisHalfSplit(lower, higher) => lower.len() + higher.len(),
        }
    }

    /// Calls the given function `f` for every leaf cell.
    pub fn for_each_leaf(&self, mut f: &mut dyn FnMut(&Vec<usize>)) {
        match &self.ty {
            CellType::Empty => {}
            CellType::Leaf(points) => f(points),
            CellType::Grid(grid) => grid.for_each_leaf(f),
            CellType::XAxisHalfSplit(lower, higher) => {
                lower.for_each_leaf(&mut f);
                higher.for_each_leaf(&mut f);
            }
        }
    }
}

/// Describes what the grid cell contains.
pub enum CellType {
    Empty,
    Leaf(Vec<usize>),
    Grid(Box<ClusterHashGrid>),
    XAxisHalfSplit(Box<CellContent>, Box<CellContent>),
}

pub struct Context<'a, 'b, 'c> {
    pub point_positions: &'a [Vector3<f32>],
    pub unique_index_counter: &'b mut AtomicUsize,
    pub plot_directory: Option<PathBuf>,
    pub debug_hash_grid_cells: Option<&'c mut Vec<AABB>>,
}

pub struct ClusterHashGrid {
    /// Maps the grid cell to the indices of the points in the point cloud.
    cells: HashMap<CellIndex, CellContent>,
    /// The size of a grid cell.
    cell_size: Vector3<f32>,
    /// The number of grid cells in each dimension. Not every grid cell might be used.
    cell_resolution: Vector3<usize>,
    /// The minimum of the bounding box of the point cloud.
    aabb: AABB,
}

impl ClusterHashGrid {
    /// Creates a new `ClusterHashGrid` with the given `point_positions` and `points_per_cell`.
    pub fn new(point_positions: &[Vector3<f32>], target_points_per_cell: usize) -> Self {
        // Every cell gets a unique index assigned to it
        let mut unique_index_counter = AtomicUsize::new(0);

        Self::from_all(
            target_points_per_cell,
            &mut Context {
                point_positions,
                unique_index_counter: &mut unique_index_counter,
                plot_directory: None,
                debug_hash_grid_cells: None,
            },
        )
    }

    /// Creates a new `ClusterHashGrid` with the given `point_positions` and `points_per_cell`.
    ///
    /// This is used when creating the highest level of the `ClusterHashGrid` hierarchy. In that
    /// case all points of the point cloud are used and the AABB of the grid is computed instead
    /// of being given. (As it is the case in the `ClusterHashGrid::from_selection` method.)
    ///
    /// # Panics
    ///
    /// * Panics if `context.point_positions` is empty.
    /// * Panics if `target_points_per_cell` is 0.
    pub fn from_all(target_points_per_cell: usize, context: &mut Context) -> Self {
        jeriya_shared::assert!(context.point_positions.len() > 0, "point_positions must not be empty");
        jeriya_shared::assert!(target_points_per_cell > 0, "target_points_per_cell must be greater than 0");

        let points_aabb = AABB::from_slice(context.point_positions);
        trace!("ClusterHashGrid points AABB: {points_aabb:?}");

        // Assuming that the density of the points is uniform, we can estimate the size of the grid cells.
        let cell_size = estimate_cell_size(&points_aabb, context.point_positions.len(), target_points_per_cell);

        // Compute the number of cells in each dimension. This might not be the same
        // as the width of the bounding box divided by the cell size because the outermost
        // cells might fall into a hash cell outside of the bounding box. If the points
        // are between 0 and 1 and the number of points suggests a cell size of 0.5, the
        // coordinate 1.0 would fall into the cell with index 2 even if there are only 2
        // cells with indices 0 and 1 in the bounding box.
        let (min_cell_index, max_cell_index) = compute_min_max_cell_index(context.point_positions.iter(), cell_size);
        let cell_resolution = (max_cell_index - min_cell_index + Vector3::new(1, 1, 1)).map(|x| x as usize);
        trace!("ClusterHashGrid cell resolution: {cell_resolution:?}");

        // Compute the bounding box of the grid. This is the bounding box that fully
        // contains all cells and not only the points. Consider the problem described above.
        let aabb_min = min_cell_index.zip_map(&cell_size, |a, b| a as f32 * b);
        let aabb_max = max_cell_index.zip_map(&cell_size, |a, b| (a + 1) as f32 * b);
        let aabb = AABB::new(aabb_min, aabb_max);
        trace!("ClusterHashGrid grid AABB: {aabb:?}");

        Self::assert_aabb_contained_in_extended_aabb(&aabb, &points_aabb);
        Self::assert_points_contained_in_extended_aabb(&aabb, context.point_positions.iter());

        let mut initial_distribution = HashMap::new();
        for (index, point_position) in context.point_positions.iter().enumerate() {
            let cell_index = Self::cell_at_point_with_cell_size(*point_position, cell_size);
            initial_distribution.entry(cell_index).or_insert_with(Vec::new).push(index);
        }
        jeriya_shared::assert_eq!(
            initial_distribution.values().map(Vec::len).sum::<usize>(),
            context.point_positions.len(),
            "all points must be in the initial distribution"
        );

        let cells = Self::build_cells_from_initial_distribution(initial_distribution, cell_size, target_points_per_cell, context);
        jeriya_shared::assert_eq!(
            cells.values().map(CellContent::len).sum::<usize>(),
            context.point_positions.len(),
            "all points must be in the cells"
        );

        Self {
            cells,
            cell_size,
            cell_resolution,
            aabb,
        }
    }

    /// Creates a new `ClusterHashGrid` within a limited space with a subset of the points.
    ///
    /// This is used when creating `ClusterHashGrid`s as part of a `ClusterHashGrid` hierarchy. In
    /// that case, the AABB of the grid is given by the cell of the parent grid. The points are a
    /// subset of the points of the parent grid.
    ///
    /// # Panics
    ///
    /// * Panics if `indices` is empty.
    /// * Panics if `target_points_per_cell` is 0.
    /// * Panics if any of the points is not in the `aabb`.
    /// * Panics if `aabb` is empty.
    pub fn from_selection(indices: &Vec<usize>, aabb: &AABB, target_points_per_cell: usize, context: &mut Context) -> Self {
        jeriya_shared::assert!(indices.len() > 0, "point_positions must not be empty");
        jeriya_shared::assert!(target_points_per_cell > 0, "target_points_per_cell must be greater than 0");
        jeriya_shared::assert!(!aabb.is_empty(), "aabb must not be empty");
        Self::assert_points_contained_in_extended_aabb(aabb, indices.iter().map(|&index| &context.point_positions[index]));

        // Estimate the size of the grid cells.
        let cell_resolution = estimate_cell_resolution(indices.len(), target_points_per_cell);
        let cell_size = aabb.size().zip_map(&cell_resolution, |a, b| a / b as f32);

        // Find a cell for every point
        let mut initial_distribution = HashMap::new();
        for index in indices {
            let point_position = context.point_positions[*index];
            let cell_index = Self::cell_at_point_with_cell_size(point_position, cell_size);
            initial_distribution.entry(cell_index).or_insert_with(Vec::new).push(*index);
        }
        jeriya_shared::assert_eq!(
            initial_distribution.values().map(|indices| indices.len()).sum::<usize>(),
            indices.len(),
            "the number of points in the initial distribution must be the same as the number of points in the selection"
        );

        // Create the final cells
        let cells = Self::build_cells_from_initial_distribution(initial_distribution, cell_size, target_points_per_cell, context);
        jeriya_shared::assert_eq!(
            cells.values().map(|cell| cell.len()).sum::<usize>(),
            indices.len(),
            "the number of points in the cells must be the same as the number of points in the selection"
        );

        Self {
            cells,
            cell_size,
            cell_resolution,
            aabb: aabb.clone(),
        }
    }

    /// Creates the cells after the points have been assigned to the cells.
    fn build_cells_from_initial_distribution(
        initial_distribution: HashMap<CellIndex, Vec<usize>>,
        cell_size: Vector3<f32>,
        target_points_per_cell: usize,
        context: &mut Context,
    ) -> HashMap<CellIndex, CellContent> {
        let mut cells = HashMap::new();
        for (cell_index, indices) in initial_distribution {
            // BoundingBox
            let aabb_min = cell_index.zip_map(&cell_size, |a, b| a as f32 * b);
            let aabb_max = aabb_min + cell_size;
            let aabb = AABB::new(aabb_min, aabb_max);

            let cell_content = Self::build_cell(indices, aabb, target_points_per_cell, context);
            cells.insert(cell_index, cell_content);
        }
        cells
    }

    /// Creates a leaf cell with the given `points`.
    ///
    /// # Panics
    ///
    /// * Panics if `points` is empty.
    fn build_leaf_cell(aabb: AABB, points: Vec<usize>, context: &mut Context) -> CellContent {
        jeriya_shared::assert!(points.len() > 0, "points must not be empty");

        // For debugging purposes, the AABB of the leaf cells can be computed and stored.
        if let Some(debug_hash_grid_cells) = &mut context.debug_hash_grid_cells {
            debug_hash_grid_cells.push(aabb);
        }

        CellContent {
            unique_index: context.unique_index_counter.fetch_add(1, Relaxed),
            aabb,
            ty: CellType::Leaf(points),
        }
    }

    /// Creates an `AABB` that is slightly larger than the given `aabb`.
    fn extended_aabb(aabb: &AABB) -> AABB {
        let epsilon = aabb.size() / 100_000.0;
        let min = aabb.min.zip_map(&epsilon, |min, epsilon| min - epsilon);
        let max = aabb.max.zip_map(&epsilon, |max, epsilon| max + epsilon);
        AABB::new(min, max)
    }

    /// Asserts that the given `aabb` after a slight extension contains all the points in the `points` iterator.
    fn assert_points_contained_in_extended_aabb<'a>(aabb: &AABB, points: impl Iterator<Item = &'a Vector3<f32>>) {
        let aabb = Self::extended_aabb(aabb);
        for point in points {
            jeriya_shared::assert!(
                aabb.contains(point),
                "AABB: {aabb:?} doesnt' contain point: {point:?}\nThere might be more points that are not contained in the AABB."
            );
        }
    }

    /// Asserts that the given `aabb` after a slight extension contains the given `other` `AABB`.
    fn assert_aabb_contained_in_extended_aabb(aabb: &AABB, other: &AABB) {
        let aabb = Self::extended_aabb(aabb);
        jeriya_shared::assert!(aabb.contains(other), "AABB: {aabb:?} doesnt' contain other: {other:?}");
    }

    fn build_cell(indices: Vec<usize>, aabb: AABB, split_threshold: usize, context: &mut Context) -> CellContent {
        Self::assert_points_contained_in_extended_aabb(&aabb, indices.iter().map(|&index| &context.point_positions[index]));

        if indices.len() > 2 * split_threshold {
            // When the number of points in a cell exceeds the `split_threshold` by a factor of 2,
            // the cell is split into a `ClusterHashGrid` recursively. The reasoning behind this is
            // that there are too many points in the cell and an easy way of splitting the points
            // was needed. The smallest possible subdivision of the `ClusterHashGrid` - apart from
            // 1x1x1 - is 2x2x2 which might result in cells with far fewer points than the
            // `split_threshold`. So, a better way of splitting the points would be beneficial.
            let grid = ClusterHashGrid::from_selection(&indices, &aabb, split_threshold, context);
            CellContent {
                unique_index: context.unique_index_counter.fetch_add(1, Relaxed),
                aabb,
                ty: CellType::Grid(Box::new(grid)),
            }
        } else if indices.len() > split_threshold {
            // When the number of points in a cell exceeds the `split_threshold` only be a factor of 2,
            // the cell is split into two cells with the same size along the x-axis. This might not be
            // the best split axis given that the distribution of points is not considered.

            // BoundingBoxes
            let x_middle = aabb.center().x;
            let lower_aabb = AABB::new(aabb.min, Vector3::new(x_middle, aabb.max.y, aabb.max.z));
            let higher_aabb = AABB::new(Vector3::new(x_middle, aabb.min.y, aabb.min.z), aabb.max);

            // Split the points into two groups
            let (lower_indices, higher_indices): (Vec<_>, Vec<_>) =
                indices.iter().partition(|index| context.point_positions[**index].x < x_middle);

            let lower = Self::build_cell(lower_indices, lower_aabb, split_threshold, context);
            let higher = Self::build_cell(higher_indices, higher_aabb, split_threshold, context);

            CellContent {
                unique_index: context.unique_index_counter.fetch_add(1, Relaxed),
                aabb,
                ty: CellType::XAxisHalfSplit(Box::new(lower), Box::new(higher)),
            }
        } else if indices.len() > 0 {
            Self::build_leaf_cell(aabb, indices, context)
        } else {
            CellContent {
                unique_index: context.unique_index_counter.fetch_add(1, Relaxed),
                aabb,
                ty: CellType::Empty,
            }
        }
    }

    /// Returns the indices of the points in the point cloud that are in the given `cell`.
    pub fn get(&self, cell: CellIndex) -> Option<&CellContent> {
        self.cells.get(&cell)
    }

    /// Returns the indices of the points in the point cloud that are in the same grid cell as the given `point`.
    pub fn get_at(&self, point: Vector3<f32>) -> Option<&CellContent> {
        self.get(self.cell_at(point)).and_then(|cell| match &cell.ty {
            CellType::Empty => None,
            CellType::Leaf(_) => Some(cell),
            CellType::Grid(grid) => grid.get_at(point),
            CellType::XAxisHalfSplit(lower, higher) => {
                if point.x < higher.aabb.min.x {
                    Some(lower.as_ref())
                } else {
                    Some(&higher.as_ref())
                }
            }
        })
    }

    /// Returns the indices of the points in the point cloud that are in the same grid cell as the given `point`.
    pub fn get_leaf(&self, point: Vector3<f32>) -> Option<(usize, &[usize])> {
        self.get_at(point).and_then(|cell| match &cell.ty {
            CellType::Empty => None,
            CellType::Leaf(points) => Some((cell.unique_index, points.as_slice())),
            CellType::Grid(grid) => grid.get_leaf(point),
            CellType::XAxisHalfSplit(lower, higher) => {
                if point.x < higher.aabb.min.x {
                    Self::get_leaf_from_cell_content(&lower, point)
                } else {
                    Self::get_leaf_from_cell_content(&higher, point)
                }
            }
        })
    }

    /// Returns the indices of the points in the point cloud that are in the same grid cell as the given `point`.
    fn get_leaf_from_cell_content(cell_content: &CellContent, point: Vector3<f32>) -> Option<(usize, &[usize])> {
        match &cell_content.ty {
            CellType::Empty => None,
            CellType::Leaf(points) => Some((cell_content.unique_index, points.as_slice())),
            CellType::Grid(grid) => grid.get_leaf(point),
            CellType::XAxisHalfSplit(lower, higher) => {
                if point.x < higher.aabb.min.x {
                    Self::get_leaf_from_cell_content(&lower, point)
                } else {
                    Self::get_leaf_from_cell_content(&higher, point)
                }
            }
        }
    }

    /// Returns the number of grid cells in the `ClusterHashGrid`.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns `true` if the `ClusterHashGrid` is empty.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Returns the grid cell that the given `point` is in.
    pub fn cell_at(&self, point: Vector3<f32>) -> CellIndex {
        Self::cell_at_point_with_cell_size(point, self.cell_size)
    }

    /// Returns the number of grid cells in each dimension.
    pub fn cell_resolution(&self) -> Vector3<usize> {
        self.cell_resolution
    }

    /// Returns the size of a grid cell.
    pub fn cell_size(&self) -> Vector3<f32> {
        self.cell_size
    }

    /// Returns an iterator over the grid cells.
    pub fn cells(&self) -> impl Iterator<Item = &CellContent> {
        self.cells.values()
    }

    /// Calls the given function `f` for every leaf cell.
    pub fn for_each_leaf(&self, mut f: &mut dyn FnMut(&Vec<usize>)) {
        for cell in self.cells() {
            cell.for_each_leaf(&mut f);
        }
    }

    /// Returns the grid cell that the given `point` is in, using the given `cell_size`.
    pub fn cell_at_point_with_cell_size(point: Vector3<f32>, cell_size: Vector3<f32>) -> CellIndex {
        let x = (point.x / cell_size.x).floor() as i32;
        let y = (point.y / cell_size.y).floor() as i32;
        let z = (point.z / cell_size.z).floor() as i32;
        CellIndex::new(x, y, z)
    }

    /// Returns the `AxisAlignedBoundingBox` of the `ClusterHashGrid`.
    pub fn bounding_box(&self) -> AABB {
        self.aabb
    }
}

/// Gives an estimate of the number of grid cells using the assumptions that the density of the points is uniform.
fn estimate_cell_resolution<'a>(point_positions_count: usize, target_points_per_cell: usize) -> Vector3<usize> {
    let point_per_dimension = (point_positions_count as f32).powf(1.0 / 3.0);
    let points_per_cell_per_dimension = (target_points_per_cell as f32).powf(1.0 / 3.0);
    let cells_per_dimension = (point_per_dimension / points_per_cell_per_dimension).ceil();
    Vector3::new(
        cells_per_dimension as usize,
        cells_per_dimension as usize,
        cells_per_dimension as usize,
    )
}

/// Gives an estimate of the size of a grid cell using the assumptions that the density of the points is uniform.
fn estimate_cell_size<'a>(aabb: &AABB, point_positions_count: usize, target_points_per_cell: usize) -> Vector3<f32> {
    let cell_resolution = estimate_cell_resolution(point_positions_count, target_points_per_cell);
    aabb.size().zip_map(&cell_resolution, |a, b| a / b as f32)
}

/// Computes the minimum and maximum cell index of the given `point_positions`.
fn compute_min_max_cell_index<'a>(
    point_positions: impl Iterator<Item = &'a Vector3<f32>>,
    cell_size: Vector3<f32>,
) -> (CellIndex, CellIndex) {
    let mut min = CellIndex::new(std::i32::MAX, std::i32::MAX, std::i32::MAX);
    let mut max = CellIndex::new(std::i32::MIN, std::i32::MIN, std::i32::MIN);
    for point in point_positions {
        let cell_index = ClusterHashGrid::cell_at_point_with_cell_size(*point, cell_size);
        min = min.zip_map(&cell_index, i32::min);
        max = max.zip_map(&cell_index, i32::max);
    }
    (min, max)
}

#[cfg(test)]
mod tests {
    use jeriya_shared::rand;

    use super::*;

    #[test]
    fn smoke() {
        let point_positions = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];
        let cluster_hash_grid = ClusterHashGrid::new(&point_positions, 2);
        assert_eq!(cluster_hash_grid.cell_resolution(), Vector3::new(2, 2, 2));
        assert_eq!(cluster_hash_grid.cell_size(), Vector3::new(0.5, 0.5, 0.5));
        assert_eq!(cluster_hash_grid.len(), 4);

        // assert_eq!(cluster_hash_grid.cell_at(Vector3::new(0.0, 0.0, 0.0)), CellIndex(0, 0, 0));
        // assert_eq!(cluster_hash_grid.cell_at(Vector3::new(1.0, 0.0, 0.0)), CellIndex(1, 0, 0));
        // assert_eq!(cluster_hash_grid.cell_at(Vector3::new(0.0, 1.0, 0.0)), CellIndex(0, 1, 0));
        // assert_eq!(cluster_hash_grid.cell_at(Vector3::new(0.0, 0.0, 1.0)), CellIndex(0, 0, 1));
    }

    #[test]
    fn more_in_one_cell() {
        let point_positions = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(0.7, 0.7, 0.7),
            Vector3::new(0.8, 0.8, 0.8),
            Vector3::new(0.9, 0.9, 0.9),
        ];
        let cluster_hash_grid = ClusterHashGrid::new(&point_positions, 4);
        assert_eq!(cluster_hash_grid.cell_resolution(), Vector3::new(2, 2, 2));
        assert_eq!(cluster_hash_grid.len(), 5);
        dbg!(cluster_hash_grid.cell_size());

        let assert_leaf_with = |x: Vector3<f32>, index: usize| {
            let CellType::Leaf(points) = &cluster_hash_grid.get_at(x).unwrap().ty else {
                panic!("Wrong CellType");
            };
            dbg!(index);
            dbg!(&points);
            assert!(points.contains(&index));
        };
        assert_leaf_with(Vector3::new(0.0, 0.0, 0.0), 0);
        assert_leaf_with(Vector3::new(1.0, 0.0, 0.0), 1);
        assert_leaf_with(Vector3::new(0.0, 1.0, 0.0), 2);
        assert_leaf_with(Vector3::new(0.0, 0.0, 1.0), 3);
        assert_leaf_with(Vector3::new(0.7, 0.7, 0.7), 4);
        assert_leaf_with(Vector3::new(0.7, 0.7, 0.7), 5); // These can be found in the same cell
        assert_leaf_with(Vector3::new(0.7, 0.7, 0.7), 6); // These can be found in the same cell
    }

    #[test]
    fn random_points() {
        env_logger::builder().filter_level(jeriya_shared::log::LevelFilter::Trace).init();

        const N: usize = 100_000;
        let random_points = (0..N)
            .map(|_| Vector3::new(rand::random(), rand::random(), rand::random()))
            .collect::<Vec<Vector3<f32>>>();
        let cluster_hash_grid = ClusterHashGrid::new(&random_points, 80);
        assert_eq!(cluster_hash_grid.cell_resolution(), Vector3::new(11, 11, 11));
    }
}
