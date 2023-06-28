use jeriya_shared::nalgebra::Matrix4;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PerFrameData {
    pub active_camera: u32,
}

impl Default for PerFrameData {
    fn default() -> Self {
        Self { active_camera: 0 }
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Camera {
    pub projection_matrix: Matrix4<f32>,
    pub view_matrix: Matrix4<f32>,
    pub matrix: Matrix4<f32>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            projection_matrix: Matrix4::identity(),
            view_matrix: Matrix4::identity(),
            matrix: Matrix4::identity(),
        }
    }
}
