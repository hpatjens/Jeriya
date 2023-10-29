use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

use jeriya_shared::{nalgebra::Vector3, rand, random_direction, ByteColor3};
use serde::{Deserialize, Serialize};

use crate::model::Model;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PointCloud {
    point_positions: Vec<Vector3<f32>>,
    point_colors: Vec<ByteColor3>,
}

impl PointCloud {
    /// Creates an empty `PointCloud`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a point cloud by sampling the surface of the given `Model`.
    pub fn sample_from_model(model: &Model, points_per_square_unit: f32) -> Self {
        // Compute the surface area of the model
        let mut surface_area = 0.0;
        for mesh in &model.meshes {
            for triangle in mesh.simple_mesh.indices.chunks(3) {
                let a = mesh.simple_mesh.vertex_positions[triangle[0] as usize];
                let b = mesh.simple_mesh.vertex_positions[triangle[1] as usize];
                let c = mesh.simple_mesh.vertex_positions[triangle[2] as usize];

                let ab = b - a;
                let ac = c - a;
                surface_area += ab.cross(&ac).norm() / 2.0;
            }
        }

        // Sample the model
        let mut point_cloud = Self::new();
        let sample_count = (surface_area * points_per_square_unit).ceil() as usize;
        for _ in 0..sample_count {
            // Pick a random mesh
            let mesh_index = rand::random::<usize>() % model.meshes.len();
            let mesh = &model.meshes[mesh_index];

            // Pick a random triangle
            let triangle_index = rand::random::<usize>() % (mesh.simple_mesh.indices.len() / 3);
            let triangle_start_index = 3 * triangle_index;
            let triangle = &mesh.simple_mesh.indices[triangle_start_index..triangle_start_index + 3];

            let a = mesh.simple_mesh.vertex_positions[triangle[0] as usize];
            let b = mesh.simple_mesh.vertex_positions[triangle[1] as usize];
            let c = mesh.simple_mesh.vertex_positions[triangle[2] as usize];
            let ab = b - a;
            let ac = c - a;

            // Sample point in parallelogram
            let u = rand::random::<f32>();
            let v = rand::random::<f32>();
            let in_triangle = u + v <= 1.0;
            let point = if in_triangle {
                a + u * ab + v * ac
            } else {
                a + (1.0 - u) * ab + (1.0 - v) * ac
            };

            // Push the point to the point cloud
            point_cloud.push(point, ByteColor3::new(255, 0, 0));
        }

        point_cloud
    }

    /// Writes the `PointCloud` to an OBJ file.
    pub fn to_obj(&self, filepath: impl AsRef<Path>) -> io::Result<()> {
        let mut file = File::create(filepath)?;

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
            const K: f32 = 0.01;
            let a = position;
            let b = position + K * u;
            let c = position + K * n;

            writeln!(file, "v {} {} {}", a.x, a.y, a.z)?;
            writeln!(file, "v {} {} {}", b.x, b.y, b.z)?;
            writeln!(file, "v {} {} {}", c.x, c.y, c.z)?;
        }

        // Writing the faces
        for index in 0..self.point_positions.len() {
            writeln!(file, "f {} {} {}", 3 * index + 1, 3 * index + 2, 3 * index + 3)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let mut point_cloud = PointCloud::new();
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
        let model = Model::import("../sample_assets/suzanne.glb").unwrap();
        let point_cloud = PointCloud::sample_from_model(&model, 100.0);
        assert_eq!(point_cloud.len(), 2646);
    }
}
