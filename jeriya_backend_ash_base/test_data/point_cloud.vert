#version 450

#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require

layout (constant_id = 0) const uint MAX_CAMERAS = 16;
layout (constant_id = 1) const uint MAX_CAMERA_INSTANCES = 64;
layout (constant_id = 2) const uint MAX_POINT_CLOUD_ATTRIBUTES = 1024;
layout (constant_id = 3) const uint MAX_RIGID_MESHES = 1024;
layout (constant_id = 4) const uint MAX_MESH_ATTRIBUTES = 1024;
layout (constant_id = 5) const uint MAX_RIGID_MESH_INSTANCES = 1024;
layout (constant_id = 6) const uint MAX_MESHLETS = 1048576;
layout (constant_id = 7) const uint MAX_VISIBLE_RIGID_MESH_INSTANCES = 1024;
layout (constant_id = 8) const uint MAX_VISIBLE_RIGID_MESH_MESHLETS = 1048576;
layout (constant_id = 9) const uint MAX_POINT_CLOUDS = 1024;
layout (constant_id = 10) const uint MAX_POINT_CLOUD_INSTANCES = 1024;

struct Camera {
    mat4 projection_matrix;
};

struct CameraInstance {
    uint64_t camera_index;
    mat4 view_matrix;
};

struct VkDrawIndirectCommand {
    uint vertex_count;
    uint instance_count;
    uint first_vertex;
    uint first_instance;
};

struct VkDispatchIndirectCommand {
    uint x;
    uint y;
    uint z;
};

// `MeshRepresentation` enum in `shader_interface.rs`
const uint MESH_REPRESENTATION_MESHLETS = 0;
const uint MESH_REPRESENTATION_SIMPLE = 1;

const uint MESHLET_MAX_VERTICES = 64;
const uint MESHLET_MAX_TRIANGLES = 126;

struct Meshlet {
    uint global_indices[MESHLET_MAX_VERTICES];
    uint local_indices[MESHLET_MAX_TRIANGLES * 3];
    uint vertex_count;
    uint triangle_count;
};

struct MeshAttributes {
    uint64_t vertex_positions_start_offset;
    uint64_t vertex_positions_len;

    uint64_t vertex_normals_start_offset;
    uint64_t vertex_normals_len;

    uint64_t indices_start_offset;
    uint64_t indices_len;

    uint64_t meshlets_start_offset;
    uint64_t meshlets_len;
};

struct PointCloudAttributes {
    uint points_len;
    uint point_positions_start_offset;
};

struct RigidMesh {
    int mesh_attributes_index;
    uint preferred_mesh_representation;
};

struct RigidMeshInstance {
    uint64_t rigid_mesh_index;
    mat4 transform;
};

struct PointCloud {
    int point_cloud_attributes_index;
};

struct PointCloudInstance {
    uint64_t point_cloud_index;
    mat4 transform;
};

layout (set = 0, binding = 0) uniform PerFrameData { 
    int active_camera_instance; // -1 means no active camera
    uint mesh_attributes_count;
    uint rigid_mesh_count;
    uint rigid_mesh_instance_count;
    uint point_cloud_instance_count;
} per_frame_data;

layout (set = 0, binding = 1) buffer Cameras { 
    Camera cameras[MAX_CAMERAS];
};

layout (set = 0, binding = 2) buffer CameraInstanceBuffer { 
    CameraInstance camera_instances[MAX_CAMERA_INSTANCES];
};

layout (set = 0, binding = 3) buffer VisibleRigidMeshInstancesSimpleBuffer { 
    uint count;
    VkDrawIndirectCommand indirect_draw_commands[MAX_RIGID_MESH_INSTANCES];
    uint rigid_mesh_instance_indices[MAX_RIGID_MESH_INSTANCES];
} visible_rigid_mesh_instances_simple;

layout (set = 0, binding = 5) buffer StaticVertexPositionBuffer {
    vec4 vertex_positions[];
};

layout (set = 0, binding = 6) buffer StaticIndexBuffer {
    uint indices[];
};

layout (set = 0, binding = 7) buffer StaticVertexNormalsBuffer {
    vec4 vertex_normals[];
};

layout (set = 0, binding = 8) buffer MeshAttributesBuffer {
    MeshAttributes mesh_attributes[MAX_MESH_ATTRIBUTES];
};

layout (set = 0, binding = 9) buffer RigidMeshes {
    RigidMesh rigid_meshes[MAX_RIGID_MESHES];
};

layout (set = 0, binding = 10) buffer MeshAttributesActiveBuffer {
    bool mesh_attributes_active[MAX_MESH_ATTRIBUTES];
};

layout (set = 0, binding = 11) buffer RigidMeshInstancesBuffer {
    RigidMeshInstance rigid_mesh_instances[MAX_RIGID_MESH_INSTANCES];
};

layout (set = 0, binding = 12) buffer StaticMeshletBuffer {
    Meshlet meshlets[MAX_MESHLETS];
};

layout (set = 0, binding = 13) buffer VisibleRigidMeshInstancesBuffer {
    VkDispatchIndirectCommand dispatch_indirect_command;
    uint count;
    uint instance_indices[MAX_VISIBLE_RIGID_MESH_INSTANCES];
} visible_rigid_mesh_instances;

layout (set = 0, binding = 14) buffer VisibleRigidMeshMeshletsBuffer {
    uint count;
    VkDrawIndirectCommand indirect_draw_commands[MAX_VISIBLE_RIGID_MESH_MESHLETS];
    uint meshlet_indices[MAX_VISIBLE_RIGID_MESH_MESHLETS];
    uint rigid_mesh_instance_indices[MAX_VISIBLE_RIGID_MESH_MESHLETS];
} visible_rigid_mesh_meshlets;

layout (set = 0, binding = 15) buffer PointCloudAttributesActiveBuffer {
    bool point_cloud_attributes_active[MAX_POINT_CLOUD_ATTRIBUTES];
};

layout (set = 0, binding = 16) buffer PointCloudBuffer {
    PointCloud point_cloud[MAX_POINT_CLOUDS];
};

layout (set = 0, binding = 17) buffer PointCloudInstanceBuffer {
    PointCloudInstance point_cloud_instances[MAX_POINT_CLOUD_INSTANCES];
};






layout (push_constant) uniform PushConstants {
    vec4 color;
    mat4 matrix;
} push_constants;

void main() {
    gl_PointSize = 1.0;
    gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
}