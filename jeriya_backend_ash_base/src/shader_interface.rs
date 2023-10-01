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
pub struct MeshAttributes {
    pub vertex_positions_start_offset: u64,
    pub vertex_positions_len: u64,

    pub vertex_normals_start_offset: u64,
    pub vertex_normals_len: u64,

    pub indices_start_offset: u64,
    // When the mesh doesn't have indices, this is 0.
    pub indices_len: u64,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct InanimateMesh {
    pub vertex_positions_start_offset: u64,
    pub vertex_positions_len: u64,

    pub vertex_normals_start_offset: u64,
    pub vertex_normals_len: u64,

    pub indices_start_offset: u64,
    // When the mesh doesn't have indices, this is 0.
    pub indices_len: u64,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct RigidMesh {
    /// Index into the [`MeshAttributes`] array. -1 means that the [`MeshAttributes`] are not available for the frame.
    pub mesh_attributes_index: i64,
}

impl Default for RigidMesh {
    fn default() -> Self {
        Self { mesh_attributes_index: -1 }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct InanimateMeshInstance {
    pub inanimate_mesh_index: u64,
    pub _padding: u64,
    pub transform: Matrix4<f32>,
}

impl Default for InanimateMeshInstance {
    fn default() -> Self {
        Self {
            inanimate_mesh_index: 0,
            _padding: 0,
            transform: Matrix4::identity(),
        }
    }
}
