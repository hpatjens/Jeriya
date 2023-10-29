use crate::model::Model;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointCloud {
    point_positions: Vec<Vector3<f32>>,
    point_colors: Vec<Vector3<f32>>,
}

impl PointCloud {
    /// Creates a point cloud by sampling the surface of the given `Model`.
    pub fn sample_from_model(model: &Model) -> Self {
        todo!()
    }
}
