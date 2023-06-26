use jeriya_shared::nalgebra::Matrix4;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PerFrameData {
    pub projection_matrix: Matrix4<f32>,
    pub view_matrix: Matrix4<f32>,
    pub matrix: Matrix4<f32>,
}

impl Default for PerFrameData {
    fn default() -> Self {
        Self {
            projection_matrix: Matrix4::identity(),
            view_matrix: Matrix4::identity(),
            matrix: Matrix4::identity(),
        }
    }
}
