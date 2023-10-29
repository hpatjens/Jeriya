use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

use jeriya_shared::{nalgebra::Vector3, random_direction, ByteColor3};
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
    pub fn sample_from_model(model: &Model) -> Self {
        let mut point_cloud = Self::new();
        for mesh in &model.meshes {
            for triangle in mesh.simple_mesh.indices.chunks(3) {
                // Vertices of the current triangle
                let a = mesh.simple_mesh.vertex_positions[triangle[0] as usize];
                let b = mesh.simple_mesh.vertex_positions[triangle[1] as usize];
                let c = mesh.simple_mesh.vertex_positions[triangle[2] as usize];
                let center = (a + b + c) / 3.0;
                point_cloud.push(center, ByteColor3::new(255, 0, 0));
            }
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
        let point_cloud = PointCloud::sample_from_model(&model);
        assert_eq!(point_cloud.len(), 1092);
    }
}
