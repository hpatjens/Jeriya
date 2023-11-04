use jeriya_backend::{elements, instances, resources};
use jeriya_shared::nalgebra::Matrix4;

pub trait Represents<T> {}

impl Represents<resources::mesh_attributes::MeshAttributes> for u32 {}
impl Represents<resources::point_cloud_attributes::PointCloudAttributes> for u32 {}

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct PerFrameData {
    pub active_camera: i32,
    pub mesh_attributes_count: u32,
    pub rigid_mesh_count: u32,
    pub rigid_mesh_instance_count: u32,
    pub point_cloud_instance_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Camera {
    pub projection_matrix: Matrix4<f32>,
}

impl Represents<elements::camera::Camera> for Camera {}

impl Default for Camera {
    fn default() -> Self {
        Self {
            projection_matrix: Matrix4::identity(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CameraInstance {
    pub camera_index: u64,
    pub _padding: u64,
    pub view_matrix: Matrix4<f32>,
}

impl Represents<instances::camera_instance::CameraInstance> for CameraInstance {}

impl Default for CameraInstance {
    fn default() -> Self {
        Self {
            camera_index: Default::default(),
            _padding: Default::default(),
            view_matrix: Matrix4::identity(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct MeshAttributes {
    pub vertex_positions_start_offset: u64,
    pub vertex_positions_len: u64,

    pub vertex_normals_start_offset: u64,
    pub vertex_normals_len: u64,

    pub indices_start_offset: u64,
    pub indices_len: u64, // When the mesh doesn't have indices, this is 0.

    pub meshlets_start_offset: u64,
    pub meshlets_len: u64, // When the mesh doesn't have meshlets, this is 0.
}

impl Represents<resources::mesh_attributes::MeshAttributes> for MeshAttributes {}

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct PointCloudAttributes {
    pub points_len: u32,
    pub point_positions_start_offset: u32,
    pub point_colors_start_offset: u32,
}

impl Represents<resources::point_cloud_attributes::PointCloudAttributes> for PointCloudAttributes {}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Meshlet {
    pub global_indices: [u32; 64],
    pub local_indices: [[u32; 3]; 126], // u8 is enough, but we need to align to 4 bytes and GLSL doesn't support u8.
    pub vertex_count: u32,              // u8 is enough, but we need to align to 4 bytes and GLSL doesn't support u8.
    pub triangle_count: u32,            // u8 is enough, but we need to align to 4 bytes and GLSL doesn't support u8.
}

impl Default for Meshlet {
    fn default() -> Self {
        Self {
            global_indices: [0; 64],
            local_indices: [[0; 3]; 126],
            triangle_count: 0,
            vertex_count: 0,
        }
    }
}

impl Represents<jeriya_content::model::Meshlet> for Meshlet {}

#[repr(u32)]
#[derive(Default, Debug, Clone, Copy)]
pub enum MeshRepresentation {
    /// When the mesh has meshlets, it will be rendered with meshlets.
    #[default]
    Meshlets = 0,
    /// Even when the mesh has meshlets, it will be rendered as a simple mesh.
    Simple = 1,
}

impl From<elements::rigid_mesh::MeshRepresentation> for MeshRepresentation {
    fn from(mesh_representation: elements::rigid_mesh::MeshRepresentation) -> Self {
        match mesh_representation {
            elements::rigid_mesh::MeshRepresentation::Meshlets => Self::Meshlets,
            elements::rigid_mesh::MeshRepresentation::Simple => Self::Simple,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct RigidMesh {
    pub mesh_attributes_index: i32,
    /// Determines how the mesh will be rendered.
    pub preferred_mesh_representation: MeshRepresentation,
}

impl Represents<elements::rigid_mesh::RigidMesh> for RigidMesh {}

impl Default for RigidMesh {
    fn default() -> Self {
        Self {
            mesh_attributes_index: -1,
            preferred_mesh_representation: MeshRepresentation::default(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct RigidMeshInstance {
    pub rigid_mesh_index: u64,
    pub _padding: u64,
    pub transform: Matrix4<f32>,
}

impl Represents<instances::rigid_mesh_instance::RigidMeshInstance> for RigidMeshInstance {}

impl Default for RigidMeshInstance {
    fn default() -> Self {
        Self {
            rigid_mesh_index: 0,
            _padding: 0,
            transform: Matrix4::identity(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PointCloud {
    pub point_cloud_attributes_index: i32,
}

impl Represents<elements::point_cloud::PointCloud> for PointCloud {}

impl Default for PointCloud {
    fn default() -> Self {
        Self {
            point_cloud_attributes_index: -1,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PointCloudInstance {
    pub point_cloud_index: u64,
    pub _padding: u64,
    pub transform: Matrix4<f32>,
}

impl Represents<instances::point_cloud_instance::PointCloudInstance> for PointCloudInstance {}

impl Default for PointCloudInstance {
    fn default() -> Self {
        Self {
            point_cloud_index: 0,
            _padding: 0,
            transform: Matrix4::identity(),
        }
    }
}
