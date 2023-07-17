use jeriya_shared::nalgebra::Matrix4;

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct PerFrameData {
    pub active_camera: u32,
    pub inanimate_mesh_instance_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Camera {
    pub projection_matrix: Matrix4<f32>,
    pub view_matrix: Matrix4<f32>,
    pub matrix: Matrix4<f32>,
}

#[repr(C)]
#[derive(Debug, Clone)]
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

#[repr(C)]
#[derive(Debug, Clone)]
pub struct InanimateMeshInstance {
    pub inanimate_mesh_index: u64,
    pub transform: Matrix4<f32>,
}

impl Default for InanimateMeshInstance {
    fn default() -> Self {
        Self {
            inanimate_mesh_index: 0,
            transform: Matrix4::identity(),
        }
    }
}
