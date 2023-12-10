#version 450

#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require
#extension GL_ARB_shader_draw_parameters : enable

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
layout (constant_id = 11) const uint MAX_POINT_CLOUD_PAGES = 16384;
// layout (constant_id = 12)
// layout (constant_id = 13)
layout (constant_id = 14) const uint MAX_VISIBLE_POINT_CLOUD_CLUSTERS = 16384;

struct FrameTelemetry {
    uint max_cameras;
    uint max_camera_instances;

    uint max_mesh_attributes;
    uint max_point_cloud_attributes;

    uint max_rigid_meshes;
    uint max_rigid_mesh_instances;
    uint max_meshlets;
    uint max_visible_rigid_mesh_instances;
    uint max_visible_rigid_mesh_meshlets;

    uint max_point_clouds;
    uint max_point_cloud_instances;
    uint max_point_cloud_pages;
    uint max_point_cloud_page_clusters;
    uint max_visible_point_cloud_clusters;

    uint visible_rigid_mesh_instances;
    uint visible_rigid_mesh_instances_simple;
    uint visible_rigid_mesh_meshlets;
    uint visible_rigid_mesh_meshlet_vertices;

    uint visible_point_cloud_instances;
    uint visible_point_cloud_instances_simple;
    uint visible_point_cloud_clusters;
};

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

// `PointCloudRepresentation` enum in `shader_interface.rs
const uint POINT_CLOUD_REPRESENTATION_CLUSTERED = 0;
const uint POINT_CLOUD_REPRESENTATION_SIMPLE = 1;

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
    uint point_colors_start_offset;
    uint pages_len;
    uint pages_start_offset;
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
    uint preferred_point_cloud_representation;
};

struct PointCloudInstance {
    uint64_t point_cloud_index;
    mat4 transform;
};

struct PointCloudCluster {
    uint points_start_offet;
    uint points_len;
};

const uint MAX_POINT_CLOUD_PAGE_POINTS = 16 * 256;
const uint MAX_POINT_CLOUD_PAGE_CLUSTERS = 16;

struct PointCloudPage {
    uint points_len;
    uint clusters_len;
    vec4 point_positions[MAX_POINT_CLOUD_PAGE_POINTS];
    vec4 point_colors[MAX_POINT_CLOUD_PAGE_POINTS];
    PointCloudCluster clusters[MAX_POINT_CLOUD_PAGE_CLUSTERS];
};

struct PointCloudClusterId {
    uint point_cloud_instance;
    uint page_index;
    uint cluster_index;
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
    bool mesh_attributes_active[MAX_MESH_ATTRIBUTES]; // bool has an alignment of 4 bytes
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
    PointCloud point_clouds[MAX_POINT_CLOUDS];
};

layout (set = 0, binding = 17) buffer PointCloudInstanceBuffer {
    PointCloudInstance point_cloud_instances[MAX_POINT_CLOUD_INSTANCES];
};

layout (set = 0, binding = 18) buffer VisiblePointCloudInstanceSimpleBuffer {
    uint count;
    VkDrawIndirectCommand indirect_draw_commands[MAX_POINT_CLOUD_INSTANCES];
    uint instance_indices[MAX_POINT_CLOUD_INSTANCES];
} visible_point_cloud_instances_simple;

layout (set = 0, binding = 19) buffer PointCloudAttributesBuffer {
    PointCloudAttributes point_cloud_attributes[MAX_POINT_CLOUD_ATTRIBUTES];
};

layout (set = 0, binding = 20) buffer StaticPointPositionBuffer {
    vec4 point_positions[];
};

layout (set = 0, binding = 21) buffer StaticPointColorBuffer {
    vec4 point_colors[];
};

layout (set = 0, binding = 22) buffer PointCloudPagesBuffer {
    PointCloudPage point_cloud_pages[MAX_POINT_CLOUD_PAGES];
};

layout (set = 0, binding = 23) buffer PointCloudPagesActiveBuffer {
    bool point_cloud_pages_active[MAX_POINT_CLOUD_PAGES];
};

layout (set = 0, binding = 24) buffer StaticPointCloudPagesBuffer {
    PointCloudPage static_point_cloud_pages[MAX_POINT_CLOUD_PAGES];
};

layout (set = 0, binding = 25) buffer VisiblePointCloudInstancesBuffer {
    VkDispatchIndirectCommand dispatch_indirect_command;
    uint count;
    uint instance_indices[MAX_POINT_CLOUD_INSTANCES];
} visible_point_cloud_instances;

layout (set = 0, binding = 26) buffer VisiblePointCloudClustersBuffer {
    VkDrawIndirectCommand draw_indirect_command;
    uint count;
    PointCloudClusterId cluster_ids[MAX_VISIBLE_POINT_CLOUD_CLUSTERS];
} visible_point_cloud_clusters;

layout (set = 0, binding = 27) buffer FrameTelemetryBuffer {
    FrameTelemetry frame_telemetry;
};


layout (location = 0) flat in PointCloudClusterId in_cluster_id;
layout (location = 3) in vec4 in_point_color;
layout (location = 4) in vec2 in_texcoord;

layout (location = 0) out vec4 outputColor;

const vec3 COLORS[] = {
    vec3(0.9020, 0.9725, 0.0000),
    vec3(0.8314, 0.9843, 0.0196),
    vec3(0.7490, 0.9647, 0.0627),
    vec3(0.6784, 0.9490, 0.1098),
    vec3(0.6157, 0.9373, 0.1529),
    vec3(0.5647, 0.9216, 0.1922),
    vec3(0.5216, 0.9098, 0.2353),
    vec3(0.4863, 0.8980, 0.2745),
    vec3(0.4627, 0.8863, 0.3137),
    vec3(0.4431, 0.8784, 0.3529),
    vec3(0.4431, 0.8784, 0.3529),
    vec3(0.3373, 0.8863, 0.2941),
    vec3(0.2314, 0.8902, 0.2431),
    vec3(0.1686, 0.9020, 0.2549),
    vec3(0.1020, 0.9137, 0.2745),
    vec3(0.0667, 0.8941, 0.3216),
    vec3(0.0471, 0.8588, 0.3765),
    vec3(0.0314, 0.8235, 0.4275),
    vec3(0.0157, 0.7843, 0.4745),
    vec3(0.0000, 0.7451, 0.5176),
    vec3(0.0000, 0.7451, 0.5176),
    vec3(0.0000, 0.7294, 0.5294),
    vec3(0.0000, 0.7098, 0.5373),
    vec3(0.0000, 0.6941, 0.5490),
    vec3(0.0000, 0.6784, 0.5569),
    vec3(0.0000, 0.6588, 0.5647),
    vec3(0.0000, 0.6431, 0.5686),
    vec3(0.0000, 0.6275, 0.5725),
    vec3(0.0000, 0.6078, 0.5765),
    vec3(0.0000, 0.5922, 0.5804),
    vec3(0.0000, 0.5922, 0.5804),
    vec3(0.0000, 0.5843, 0.5843),
    vec3(0.0000, 0.5647, 0.5765),
    vec3(0.0000, 0.5451, 0.5647),
    vec3(0.0000, 0.5255, 0.5569),
    vec3(0.0000, 0.5059, 0.5490),
    vec3(0.0000, 0.4902, 0.5412),
    vec3(0.0000, 0.4706, 0.5294),
    vec3(0.0000, 0.4510, 0.5216),
    vec3(0.0000, 0.4353, 0.5137),
    vec3(0.0000, 0.4353, 0.5137),
    vec3(0.0196, 0.4118, 0.4941),
    vec3(0.0392, 0.3882, 0.4784),
    vec3(0.0588, 0.3647, 0.4588),
    vec3(0.0824, 0.3490, 0.4392),
    vec3(0.1020, 0.3294, 0.4196),
    vec3(0.1216, 0.3137, 0.4039),
    vec3(0.1412, 0.3020, 0.3843),
    vec3(0.1647, 0.2902, 0.3647),
    vec3(0.1843, 0.2824, 0.3451),
};

uint esgtsa(uint s) {
    s = (s ^ 2747636419u) * 2654435769u;// % 4294967296u;
    s = (s ^ (s >> 16u)) * 2654435769u;// % 4294967296u;
    s = (s ^ (s >> 16u)) * 2654435769u;// % 4294967296u;
    return s;
}

void main() {
    const float extent_down = 0.288675;
    if (length(in_texcoord) > extent_down) {
        discard;
        return;
    }
    outputColor = in_point_color;
}
