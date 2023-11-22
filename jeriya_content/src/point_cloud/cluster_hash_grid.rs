use std::{collections::HashMap, path::Path};

use jeriya_shared::{
    log::{info, trace},
    nalgebra::Vector3,
    plotters::{
        backend::{DrawingBackend, SVGBackend},
        chart::ChartBuilder,
        drawing::DrawingAreaErrorKind,
        prelude::*,
    },
};

use super::bounding_box::AABB;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellIndex(pub i32, pub i32, pub i32);

pub enum CellContent {
    Points(Vec<usize>),
    Grid(Box<ClusterHashGrid>),
    HalfSplit(Box<CellContent>, Box<CellContent>),
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
    /// The number of points per cluster that should not be exceeded.
    target_points_per_cell: usize,
}

impl ClusterHashGrid {
    /// Creates a new `ClusterHashGrid` with the given `point_positions` and `points_per_cell`.
    pub fn new(point_positions: &[Vector3<f32>], target_points_per_cell: usize) -> Self {
        Self::with_debug_options(point_positions, target_points_per_cell, None)
    }

    /// Creates a new `ClusterHashGrid` with the given `point_positions` and `points_per_cell`.
    pub fn with_debug_options(point_positions: &[Vector3<f32>], target_points_per_cell: usize, plot_directory: Option<&Path>) -> Self {
        assert!(point_positions.len() > 0, "point_positions must not be empty");

        let mut initial_distribution = HashMap::new();

        // Compute Bounding Box
        let aabb = AABB::from_slice(point_positions);
        info!("ClusterHashGrid Bounding Box: {aabb:?}");

        // Assuming that the density of the points is uniform, we can compute the number of cells.
        let point_per_dimension = (point_positions.len() as f32).powf(1.0 / 3.0);
        let points_per_cell_per_dimension = (target_points_per_cell as f32).powf(1.0 / 3.0);
        let cells_per_dimension = (point_per_dimension / points_per_cell_per_dimension).ceil();
        let cell_size = (aabb.max - aabb.min) / cells_per_dimension;
        let cell_resolution = Vector3::new(
            cells_per_dimension as usize,
            cells_per_dimension as usize,
            cells_per_dimension as usize,
        );
        info! {
            "ClusterHashGrid cell_size: {:?} (clusters_per_dimension: {:?})",
            cell_size, cells_per_dimension
        }

        // Insert the points into cells
        for (point_index, point_position) in point_positions.iter().enumerate() {
            let cell_index = Self::cell_at_point_with_cell_size(*point_position, cell_size);
            initial_distribution.entry(cell_index).or_insert_with(Vec::new).push(point_index);
        }

        // Recursively splits the cells that have too many points
        let mut cells = HashMap::new();
        for (cell_index, points) in initial_distribution {
            let split_threshold = target_points_per_cell;
            let cell_content = Self::split_cell(points, &point_positions, split_threshold, plot_directory);
            cells.insert(cell_index, cell_content);
        }

        Self {
            cells,
            cell_size,
            aabb,
            cell_resolution,
            target_points_per_cell,
        }
    }

    fn split_cell(
        points: Vec<usize>,
        point_positions: &[Vector3<f32>],
        split_threshold: usize,
        plot_directory: Option<&Path>,
    ) -> CellContent {
        if points.len() > 2 * split_threshold {
            trace!("Splitting with {} points", points.len());
            let point_positions = points.iter().map(|&point_index| point_positions[point_index]).collect::<Vec<_>>();
            let grid = ClusterHashGrid::with_debug_options(&point_positions, split_threshold, plot_directory);
            CellContent::Grid(Box::new(grid))
        } else if points.len() > split_threshold {
            let mut point_positions = points
                .iter()
                .map(|&point_index| (point_index, point_positions[point_index]))
                .collect::<Vec<_>>();
            point_positions.sort_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
            let (first, second) = point_positions.split_at(points.len() / 2);
            let first = CellContent::Points(first.iter().map(|(index, _)| *index).collect());
            let second = CellContent::Points(second.iter().map(|(index, _)| *index).collect());
            CellContent::HalfSplit(Box::new(first), Box::new(second))
        } else {
            CellContent::Points(points)
        }
    }

    /// Returns the indices of the points in the point cloud that are in the given `cell`.
    pub fn get(&self, cell: CellIndex) -> Option<&CellContent> {
        self.cells.get(&cell)
    }

    /// Returns the indices of the points in the point cloud that are in the same grid cell as the given `point`.
    pub fn get_at(&self, point: Vector3<f32>) -> Option<&CellContent> {
        self.get(self.cell_at(point))
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

    /// Plots the distribution of points in the grid cells.
    pub fn plot_point_distribution_in_cells(
        &self,
        filepath: &impl AsRef<Path>,
    ) -> Result<(), DrawingAreaErrorKind<<SVGBackend as DrawingBackend>::ErrorType>> {
        plot_point_distribution_in_cells(&self.cells, self.target_points_per_cell, filepath)
    }

    /// Returns the grid cell that the given `point` is in, using the given `cell_size`.
    pub fn cell_at_point_with_cell_size(point: Vector3<f32>, cell_size: Vector3<f32>) -> CellIndex {
        let x = (point.x / cell_size.x).floor() as i32;
        let y = (point.y / cell_size.y).floor() as i32;
        let z = (point.z / cell_size.z).floor() as i32;
        CellIndex(x, y, z)
    }

    /// Returns the `AxisAlignedBoundingBox` of the `ClusterHashGrid`.
    pub fn bounding_box(&self) -> AABB {
        self.aabb
    }
}

fn plot_point_distribution_in_cells<'a>(
    map: &HashMap<CellIndex, CellContent>,
    target_points_per_cell: usize,
    filepath: &impl AsRef<Path>,
) -> Result<(), DrawingAreaErrorKind<<SVGBackend<'a> as DrawingBackend>::ErrorType>> {
    let drawing_area = SVGBackend::new(filepath, (800, 600)).into_drawing_area();

    drawing_area.fill(&WHITE)?;

    // Collect the data from the cells
    let mut data = HashMap::<u32, u32>::new();
    fn insert_points<'a>(data_to_plot: &mut HashMap<u32, u32>, cells: impl Iterator<Item = &'a CellContent>) {
        for cell in cells {
            match cell {
                CellContent::Points(points) => {
                    data_to_plot
                        .entry(points.len() as u32)
                        .and_modify(|count| *count += 1)
                        .or_insert(1u32);
                }
                CellContent::Grid(grid) => insert_points(data_to_plot, grid.cells.values()),
                CellContent::HalfSplit(first, second) => {
                    insert_points(data_to_plot, std::iter::once(first.as_ref()));
                    insert_points(data_to_plot, std::iter::once(second.as_ref()));
                }
            }
        }
    }
    insert_points(&mut data, map.values());

    // Convert the data into the representation that plotters expects
    let mut data = data.into_iter().collect::<Vec<_>>();
    data.sort_by(|(a, _), (b, _)| a.cmp(b));

    let x_max = data.iter().map(|(a, _)| a).max().cloned().unwrap_or(0u32);
    let y_max = data.iter().map(|(_, b)| b).max().cloned().unwrap_or(0u32);

    let mut chart = ChartBuilder::on(&drawing_area)
        .x_label_area_size(35)
        .y_label_area_size(40)
        .margin(5)
        .caption("Histogram of Points per Cell", ("sans-serif", 50.0))
        .build_cartesian_2d((0u32..x_max).into_segmented(), 0u32..y_max)?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .y_desc("Count")
        .x_desc("Points per Cell")
        .axis_desc_style(("sans-serif", 15))
        .draw()?;

    let (less, more): (Vec<_>, Vec<_>) = data
        .into_iter()
        .partition(|(points_per_cell, _cell_count)| *points_per_cell <= target_points_per_cell as u32);
    let less_style = BLUE.mix(0.5).filled();
    let more_style = RED.mix(0.5).filled();
    chart.draw_series(Histogram::vertical(&chart).margin(1).style(less_style).data(less))?;
    chart.draw_series(Histogram::vertical(&chart).margin(1).style(more_style).data(more))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use jeriya_shared::{function_name, rand};
    use jeriya_test::create_test_result_folder_for_function;

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
        assert_eq!(cluster_hash_grid.len(), 4);

        let directory = create_test_result_folder_for_function(function_name!());
        cluster_hash_grid
            .plot_point_distribution_in_cells(&directory.join("histogram_of_points_per_cell.svg"))
            .unwrap();
    }

    #[test]
    fn remove_cells_via_flood_fill() {
        env_logger::builder().filter_level(jeriya_shared::log::LevelFilter::Trace).init();

        const N: usize = 100_000;
        let random_points = (0..N)
            .map(|_| Vector3::new(rand::random(), rand::random(), rand::random()))
            .collect::<Vec<Vector3<f32>>>();
        let cluster_hash_grid = ClusterHashGrid::new(&random_points, 80);
        // assert_eq!(cluster_hash_grid.cell_resolution(), Vector3::new(7, 7, 7));

        let directory = create_test_result_folder_for_function(function_name!());
        cluster_hash_grid
            .plot_point_distribution_in_cells(&directory.join("histogram_of_points_per_cell.svg"))
            .unwrap();
    }
}
