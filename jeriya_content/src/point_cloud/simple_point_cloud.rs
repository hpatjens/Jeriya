use std::{
    collections::HashMap,
    io::{self, Write},
};

use jeriya_shared::{float_cmp::approx_eq, log::info, nalgebra::Vector3, rand, random_direction, ByteColor3};
use serde::{Deserialize, Serialize};

use crate::model::Model;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SimplePointCloud {
    point_positions: Vec<Vector3<f32>>,
    point_colors: Vec<ByteColor3>,
}

impl SimplePointCloud {
    /// Creates an empty `PointCloud`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a point cloud by sampling the surface of the given `Model`.
    pub fn sample_from_model(model: &Model, points_per_square_unit: f32) -> Self {
        let triangle_count = model.meshes.iter().map(|mesh| mesh.simple_mesh.indices.len() / 3).sum::<usize>();
        info!("Mesh count: {}", model.meshes.len());
        info!("Triangle count: {}", triangle_count);

        // Compute the surface areas of the meshes and the triangles in the meshes
        let surface_areas = SurfaceAreas::compute_for(model);
        for (mesh_index, surface_area) in surface_areas.mesh_surface_areas.iter().enumerate() {
            info!("Mesh {mesh_index} surface area: {surface_area}");
        }
        info!("Surface areas of triangles are omitted");

        // Compute the cumulative sums to be able to use them as a sampling distribution
        let cumulative_sums = CumulativeSums::compute_for(&surface_areas);
        for (mesh_index, cumulative_sum) in cumulative_sums.mesh_cumulative_sums.iter().enumerate() {
            info!("Mesh {mesh_index} cumulative sum: {cumulative_sum}");
        }
        info!("Cumulative sums of triangles are omitted");

        // Determine how many sample points to generate
        let sample_count = (surface_areas.overall_surface_area * points_per_square_unit).ceil() as usize;
        info!("Surface area: {}", surface_areas.overall_surface_area);
        info!("Sample count: {}", sample_count);

        // Sample the model
        let mut point_cloud = Self::new();
        for _ in 0..sample_count {
            // Pick a random mesh
            let mesh_random = rand::random::<f32>();
            let mesh_index = index_from_cumulative_sums(&cumulative_sums.mesh_cumulative_sums, mesh_random);
            let mesh = &model.meshes[mesh_index];

            // Pick a random triangle
            let triangle_random = rand::random::<f32>();
            let triangle_index = index_from_cumulative_sums(&cumulative_sums.all_triangle_cumulative_sums[&mesh_index], triangle_random);
            let triangle_start_index = 3 * triangle_index;
            let triangle = &mesh.simple_mesh.indices[triangle_start_index..triangle_start_index + 3];

            let a = mesh.simple_mesh.vertex_positions[triangle[0] as usize];
            let b = mesh.simple_mesh.vertex_positions[triangle[1] as usize];
            let c = mesh.simple_mesh.vertex_positions[triangle[2] as usize];
            let ab = b - a;
            let ac = c - a;

            // Sample in parallelogram
            let alpha = rand::random::<f32>();
            let beta = rand::random::<f32>();
            let in_triangle = alpha + beta <= 1.0;

            // Compute the point position
            let point_position = if in_triangle {
                a + alpha * ab + beta * ac
            } else {
                a + (1.0 - alpha) * ab + (1.0 - beta) * ac
            };

            // Sample the point color
            const MISSING_COLOR: ByteColor3 = ByteColor3::new(255, 0, 0);
            let point_color = if let Some(vertex_texture_coordinates) = &mesh.simple_mesh.vertex_texture_coordinates {
                let uv_a = vertex_texture_coordinates[triangle[0] as usize];
                let uv_b = vertex_texture_coordinates[triangle[1] as usize];
                let uv_c = vertex_texture_coordinates[triangle[2] as usize];
                let uv_ab = uv_b - uv_a;
                let uv_ac = uv_c - uv_a;
                let uv = if in_triangle {
                    uv_a + alpha * uv_ab + beta * uv_ac
                } else {
                    uv_a + (1.0 - alpha) * uv_ab + (1.0 - beta) * uv_ac
                };
                if let Some(material_index) = mesh.simple_mesh.material_index {
                    let material = &model.materials[material_index];
                    if let Some(base_color_texture_index) = &material.base_color_texture_index {
                        let base_color_texture = &model.textures[*base_color_texture_index];
                        base_color_texture.sample(uv).as_byte_color3()
                    } else {
                        material.base_color_color.as_byte_color3()
                    }
                } else {
                    MISSING_COLOR
                }
            } else {
                MISSING_COLOR
            };

            // Push the point to the point cloud
            point_cloud.push(point_position, point_color);
        }

        point_cloud
    }

    /// Writes the `PointCloud` to an OBJ file.
    pub fn to_obj(&self, mut obj_writer: impl Write, point_size: f32) -> io::Result<()> {
        // Writing the vertex positions
        for position in &self.point_positions {
            // Creating a coordinate system
            let u = random_direction();
            let mut v = random_direction();
            while v == u {
                v = random_direction();
            }
            let n = u.cross(&v).normalize();

            // Creating a triangle
            let a = position;
            let b = position + point_size * u;
            let c = position + point_size * n;

            writeln!(obj_writer, "v {} {} {}", a.x, a.y, a.z)?;
            writeln!(obj_writer, "v {} {} {}", b.x, b.y, b.z)?;
            writeln!(obj_writer, "v {} {} {}", c.x, c.y, c.z)?;
        }

        // Writing the faces
        for index in 0..self.point_positions.len() {
            writeln!(obj_writer, "f {} {} {}", 3 * index + 1, 3 * index + 2, 3 * index + 3)?;
        }

        Ok(())
    }

    /// Returns the positions of the points in the `PointCloud`.
    pub fn point_positions(&self) -> &[Vector3<f32>] {
        &self.point_positions
    }

    /// Returns the colors of the points in the `PointCloud`.
    pub fn point_colors(&self) -> &[ByteColor3] {
        &self.point_colors
    }

    /// Pushes a point to the `PointCloud`.
    pub fn push(&mut self, position: Vector3<f32>, color: ByteColor3) {
        self.point_positions.push(position);
        self.point_colors.push(color);
    }

    /// Returns the number of points in the `PointCloud`.
    pub fn len(&self) -> usize {
        jeriya_shared::assert!(self.point_positions.len() == self.point_colors.len());
        self.point_positions.len()
    }

    /// Returns `true` if the `PointCloud` contains no points.
    pub fn is_empty(&self) -> bool {
        self.point_positions.is_empty()
    }
}

#[derive(Default, Debug, Clone)]
struct SurfaceAreas {
    overall_surface_area: f32,
    mesh_surface_areas: Vec<f32>,
    /// Maps mesh index to `Vec` of triangle surface areas
    all_triangle_surface_areas: HashMap<usize, Vec<f32>>,
}

impl SurfaceAreas {
    /// Computes the surface areas of the given `Model`.
    fn compute_for(model: &Model) -> Self {
        let mut surface_areas = SurfaceAreas::default();
        for (mesh_index, mesh) in model.meshes.iter().enumerate() {
            let mut mesh_surface_area = 0.0; // surface area of this mesh
            let mut triangle_surface_areas = Vec::new(); // surface areas of each triangle in this mesh

            for triangle in mesh.simple_mesh.indices.chunks(3) {
                let a = mesh.simple_mesh.vertex_positions[triangle[0] as usize];
                let b = mesh.simple_mesh.vertex_positions[triangle[1] as usize];
                let c = mesh.simple_mesh.vertex_positions[triangle[2] as usize];

                let ab = b - a;
                let ac = c - a;
                let area = ab.cross(&ac).norm() / 2.0;
                surface_areas.overall_surface_area += area;
                mesh_surface_area += area;
                triangle_surface_areas.push(area);
            }

            surface_areas.mesh_surface_areas.push(mesh_surface_area);
            surface_areas.all_triangle_surface_areas.insert(mesh_index, triangle_surface_areas);
        }
        surface_areas
    }
}

#[derive(Default, Debug, Clone)]
struct CumulativeSums {
    mesh_cumulative_sums: Vec<f32>,
    /// Maps mesh index to `Vec` of triangle cumulative sums
    all_triangle_cumulative_sums: HashMap<usize, Vec<f32>>,
}

impl CumulativeSums {
    fn compute_for(surface_areas: &SurfaceAreas) -> Self {
        // Compute sampling probabilities
        let mesh_cumulative_sums = compute_cumulative_sums(&surface_areas.mesh_surface_areas);
        let all_triangle_cumulative_sums = surface_areas
            .all_triangle_surface_areas
            .iter()
            .map(|(&mesh_index, triangle_surface_areas)| {
                let cumulative_sums = compute_cumulative_sums(triangle_surface_areas);
                (mesh_index, cumulative_sums)
            })
            .collect::<_>();
        Self {
            mesh_cumulative_sums,
            all_triangle_cumulative_sums,
        }
    }
}

/// Returns the index into the given `Vec` of cumulative sums for the given random number.
fn index_from_cumulative_sums(cumulative_sums: &[f32], random: f32) -> usize {
    cumulative_sums
        .binary_search_by(|cumulative_sum| {
            cumulative_sum
                .partial_cmp(&random)
                .expect("failed to compare random value with cumulative sum")
        })
        .unwrap_or_else(|index| index)
        .min(cumulative_sums.len() - 1)
}

// Computes the cumulative sums of the given surface areas. The resulting `Vec` has the same
// length as the given `Vec` but contains the cumulative sums normalized to 1.0. The last value
// is always ~1.0 while the remaining values are < 1.0.
fn compute_cumulative_sums(surface_areas: &[f32]) -> Vec<f32> {
    let overall_surface_area = surface_areas.iter().sum::<f32>();
    let mut probabilities = Vec::with_capacity(surface_areas.len());
    let mut probability_sum = 0.0;
    for surface_area in surface_areas {
        probability_sum += surface_area / overall_surface_area;
        probabilities.push(probability_sum);
    }
    jeriya_shared::assert!(approx_eq!(f32, probability_sum, 1.0, epsilon = 0.01));
    probabilities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let mut point_cloud = SimplePointCloud::new();
        assert!(point_cloud.is_empty());

        point_cloud.push(Vector3::new(1.0, 2.0, 3.0), ByteColor3::new(4, 5, 6));
        point_cloud.push(Vector3::new(7.0, 8.0, 9.0), ByteColor3::new(10, 11, 12));

        assert!(!point_cloud.is_empty());
        assert_eq!(point_cloud.len(), 2);
        assert_eq!(
            point_cloud.point_positions(),
            &[Vector3::new(1.0, 2.0, 3.0), Vector3::new(7.0, 8.0, 9.0)]
        );
        assert_eq!(point_cloud.point_colors(), &[ByteColor3::new(4, 5, 6), ByteColor3::new(10, 11, 12)]);
    }

    #[test]
    fn sample_from_model() {
        let model = Model::import("../sample_assets/models/suzanne.glb").unwrap();
        let point_cloud = SimplePointCloud::sample_from_model(&model, 100.0);
        assert_eq!(point_cloud.len(), 2646);
    }

    #[test]
    fn index_from_cumulative_sums_smoke() {
        let cumulative_sums = vec![0.1, 0.2, 0.7, 1.0];
        assert_eq!(index_from_cumulative_sums(&cumulative_sums, -0.1), 0);
        assert_eq!(index_from_cumulative_sums(&cumulative_sums, 0.0), 0);
        assert_eq!(index_from_cumulative_sums(&cumulative_sums, 0.2), 1);
        assert_eq!(index_from_cumulative_sums(&cumulative_sums, 0.8), 3);
        assert_eq!(index_from_cumulative_sums(&cumulative_sums, 1.1), 3);
    }

    mod surface_areas {
        use super::*;

        #[test]
        fn smoke() {
            let model = Model::import("../sample_assets/models/suzanne.glb").unwrap();
            let surface_areas = SurfaceAreas::compute_for(&model);
            assert!(approx_eq!(f32, surface_areas.overall_surface_area, 26.453384, ulps = 2));
            // mesh surface areas
            assert!(approx_eq!(f32, surface_areas.mesh_surface_areas[0], 13.992528, ulps = 2));
            assert!(approx_eq!(f32, surface_areas.mesh_surface_areas[1], 12.460879, ulps = 2));
            // triangle surface areas
            let mesh0 = &surface_areas.all_triangle_surface_areas[&0];
            assert!(approx_eq!(f32, mesh0[0], 0.027444806, ulps = 2));
            assert!(approx_eq!(f32, mesh0[1], 0.027444806, ulps = 2));
            assert!(approx_eq!(f32, mesh0[2], 0.027444808, ulps = 2));
            let mesh1 = &surface_areas.all_triangle_surface_areas[&1];
            assert!(approx_eq!(f32, mesh1[0], 0.0049548564, ulps = 2));
            assert!(approx_eq!(f32, mesh1[1], 0.012353032, ulps = 2));
            assert!(approx_eq!(f32, mesh1[2], 0.009461528, ulps = 2));
        }
    }

    mod cumulative_sums {
        use jeriya_shared::float_cmp::assert_approx_eq;

        use super::*;

        #[test]
        fn smoke() {
            let model = Model::import("../sample_assets/models/suzanne.glb").unwrap();
            let surface_areas = SurfaceAreas::compute_for(&model);
            let cumulative_sums = CumulativeSums::compute_for(&surface_areas);
            // mesh cumulative sums
            assert_approx_eq!(f32, cumulative_sums.mesh_cumulative_sums[0], 0.5289499, ulps = 2);
            assert_approx_eq!(f32, cumulative_sums.mesh_cumulative_sums[1], 1.0, ulps = 2);
            // triangle cumulative sums
            let mesh0 = &cumulative_sums.all_triangle_cumulative_sums[&0];
            assert_approx_eq!(f32, mesh0[0], 0.00196139, ulps = 2);
            assert_approx_eq!(f32, mesh0[1], 0.00392278, ulps = 2);
            assert_approx_eq!(f32, mesh0[2], 0.0058841705, ulps = 2);
            let mesh1 = &cumulative_sums.all_triangle_cumulative_sums[&1];
            assert_approx_eq!(f32, mesh1[0], 0.00039763298, ulps = 2);
            assert_approx_eq!(f32, mesh1[1], 0.0013889781, ulps = 2);
            assert_approx_eq!(f32, mesh1[2], 0.0021482767, ulps = 2);
        }
    }
}
