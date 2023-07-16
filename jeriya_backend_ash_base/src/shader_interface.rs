use jeriya_shared::nalgebra::Matrix4;

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct PerFrameData {
    pub active_camera: u32,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Camera {
    pub projection_matrix: Matrix4<f32>,
    pub view_matrix: Matrix4<f32>,
    pub matrix: Matrix4<f32>,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct InanimateMesh {
    pub start_offset: u64,
    pub vertices_len: u64,
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
