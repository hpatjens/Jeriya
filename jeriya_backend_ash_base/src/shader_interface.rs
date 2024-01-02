use jeriya_backend::{elements, instances, resources};
use jeriya_content::point_cloud::clustered_point_cloud::Page;
use jeriya_shared::nalgebra::{Matrix4, Vector4};

pub trait Represents<T> {}

impl Represents<resources::mesh_attributes::MeshAttributes> for u32 {}
impl Represents<resources::point_cloud_attributes::PointCloudAttributes> for u32 {}

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct FrameTelemetry {
    max_cameras: u32,
    max_camera_instances: u32,

    max_mesh_attributes: u32,
    max_point_cloud_attributes: u32,

    max_rigid_meshes: u32,
    max_rigid_mesh_instances: u32,
    max_meshlets: u32,
    max_visible_rigid_mesh_instances: u32,
    max_visible_rigid_mesh_meshlets: u32,

    max_point_clouds: u32,
    max_point_cloud_instances: u32,
    max_point_cloud_pages: u32,
    max_point_cloud_page_clusters: u32,
    max_visible_point_cloud_clusters: u32,

    visible_rigid_mesh_instances: u32,
    visible_rigid_mesh_instances_simple: u32,
    visible_rigid_mesh_meshlets: u32,
    /// Number of vertices in the visible meshlets. This is not the number of vertices stored in the visible meshlets but the number of indices referencing vertices and therefore the number of rendered vertices.
    visible_rigid_mesh_meshlet_vertices: u32,

    visible_point_cloud_instances: u32,
    visible_point_cloud_instances_simple: u32,
    visible_point_cloud_clusters: u32,
}

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
    pub pages_len: u32,
    pub pages_start_offset: u32,

    /// Index of the page in which the root cluster is located.
    pub root_cluster_page_index: u32,
    /// Index of the root cluster in the page in which it is located.
    pub root_cluster_cluster_index: u32,
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

#[repr(u32)]
#[derive(Default, Debug, Clone, Copy)]
pub enum PointCloudRepresentation {
    /// When the point cloud has a point cloud attributes, it will be rendered with the point cloud attributes.
    #[default]
    Clustered = 0,
    /// When the point cloud has a reference to a point cloud page, it will be rendered with the point cloud page.
    Simple = 1,
}

impl From<elements::point_cloud::PointCloudRepresentation> for PointCloudRepresentation {
    fn from(point_cloud_representation: elements::point_cloud::PointCloudRepresentation) -> Self {
        match point_cloud_representation {
            elements::point_cloud::PointCloudRepresentation::Clustered => Self::Clustered,
            elements::point_cloud::PointCloudRepresentation::Simple => Self::Simple,
        }
    }
}

#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct PointCloudCluster {
    /// Center in xyz and radius in w
    pub center_radius: Vector4<f32>,
    /// Index of the first point belonging to this cluster in the `PointCloudPage`
    pub points_start_offset: u32,
    /// Number of points belonging to this cluster in the `PointCloudPage`
    pub points_len: u32,
    /// Level of this cluster in the cluster hierarchy. 0 is the leaf cluster.
    pub level: u32,
    /// Depth of this cluster in the cluster hierarchy. 0 is the root cluster.
    pub depth: u32,
    /// Number of children of this cluster.
    pub children_count: u32,
    /// Indices of the pages containing the children of this cluster.
    pub children_page_indices: [u32; 2],
    /// Indices of the clusters inside the pages containing the children of this cluster.
    pub children_cluster_indices: [u32; 2],
    /// Padding to achieve 16 bytes alignment. Because the largest member in GLSL is vec4 leading to 16 bytes alignment.
    pub padding: [u32; 3],
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PointCloudPage {
    /// Number of points in this page. The `point_positions` array must only have this many elements.
    pub points_len: u32,
    /// Number of clusters in this page. The `clusters` array must only have this many elements.
    pub clusters_len: u32,
    pub _padding: [u32; 2],
    pub point_positions: [Vector4<f32>; Page::MAX_POINTS],
    pub point_colors: [Vector4<f32>; Page::MAX_POINTS],
    pub clusters: [PointCloudCluster; Page::MAX_CLUSTERS],
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PointCloud {
    pub point_cloud_attributes_index: i32,
    /// Determines how the point cloud will be rendered.
    pub preferred_point_cloud_representation: PointCloudRepresentation,
}

impl Represents<elements::point_cloud::PointCloud> for PointCloud {}

impl Default for PointCloud {
    fn default() -> Self {
        Self {
            point_cloud_attributes_index: -1,
            preferred_point_cloud_representation: PointCloudRepresentation::default(),
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

#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct PointCloudClusterId {
    pub page_index: u32,
    pub cluster_index: u32,
}
