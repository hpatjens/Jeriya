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
layout (constant_id = 14) const uint MAX_VISIBLE_POINT_CLOUD_CLUSTERS = 1048576;
layout (constant_id = 15) const uint MAX_DEVICE_LOCAL_DEBUG_LINES_COMPONENT_COUNT = 16384;

const float TAU = 6.283184;
const float PI = 3.141592;



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
    float znear;
    float zfar;
    float padding[14];
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
    uint root_cluster_page_index;
    uint root_cluster_cluster_index;
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

// alignment of 16 bytes
struct PointCloudCluster {
    vec4 center_radius;                     // 16 bytes     0-15
    uint points_start_offet;                // 4 bytes      16-19
    uint points_len;                        // 4 bytes      20-23
    uint level;                             // 4 bytes      24-27
    uint depth;                             // 4 bytes      28-31
    uint children_count;                    // 4 bytes      32-35
    uint children_page_indices[2];          // 8 bytes      36-43
    uint children_cluster_indices[2];       // 8 bytes      44-51
    uint padding[3];                        // 12 bytes     52-63
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
    uint framebuffer_width;
    uint framebuffer_height;
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
    uint count;
    VkDrawIndirectCommand draw_indirect_commands[MAX_VISIBLE_POINT_CLOUD_CLUSTERS];
    PointCloudClusterId cluster_ids[MAX_VISIBLE_POINT_CLOUD_CLUSTERS];
} visible_point_cloud_clusters;

layout (set = 0, binding = 27) buffer FrameTelemetryBuffer {
    FrameTelemetry frame_telemetry;
};

layout (set = 0, binding = 28) buffer DeviceLocalDebugLineBuffer {
    uint count; // this is the requested number of lines which might be higher than the actually draw number
    VkDrawIndirectCommand draw_indirect_command;
    float lines[MAX_DEVICE_LOCAL_DEBUG_LINES_COMPONENT_COUNT];
} device_local_debug_lines;

const uint DEVICE_LOCAL_DEBUG_LINES_COMPONENTS_PER_LINE = 10; // 3 start, 3 end, 4 color

/// Pushes a debug line to the device local debug line buffer. These lines will be rendered at the end of the frame.
/// It might not be possible to draw lines from all shaders with correct synchronization.
struct AABB2 {
    vec2 min;
    vec2 max;
};

void push_debug_line(vec3 start, vec3 end, vec4 color) {
    uint index = atomicAdd(device_local_debug_lines.count, 1);
    if (index >= MAX_DEVICE_LOCAL_DEBUG_LINES_COMPONENT_COUNT) {
        // It is expected that device_local_debug_lines.count contains the number of actually written lines.
        atomicMax(device_local_debug_lines.count, MAX_DEVICE_LOCAL_DEBUG_LINES_COMPONENT_COUNT);
        return;
    }

    atomicAdd(device_local_debug_lines.draw_indirect_command.vertex_count, 2);
    device_local_debug_lines.draw_indirect_command.instance_count = 1;
    device_local_debug_lines.draw_indirect_command.first_vertex = 0;
    device_local_debug_lines.draw_indirect_command.first_instance = 0;

    const uint C = DEVICE_LOCAL_DEBUG_LINES_COMPONENTS_PER_LINE;
    device_local_debug_lines.lines[C * index + 0] = start.x;
    device_local_debug_lines.lines[C * index + 1] = start.y;
    device_local_debug_lines.lines[C * index + 2] = start.z;
    device_local_debug_lines.lines[C * index + 3] = end.x;
    device_local_debug_lines.lines[C * index + 4] = end.y;
    device_local_debug_lines.lines[C * index + 5] = end.z;
    device_local_debug_lines.lines[C * index + 6] = color.r;
    device_local_debug_lines.lines[C * index + 7] = color.g;
    device_local_debug_lines.lines[C * index + 8] = color.b;
    device_local_debug_lines.lines[C * index + 9] = color.a;
}

/// Pushes a rectangle in ndc on the xy plane with z = 0.0 to the debug line buffer.
void push_debug_ndc_aabb2(AABB2 aabb, vec4 min_color, vec4 max_color) {
    vec3 ll = vec3(aabb.min.x, aabb.min.y, 0.0); // lower left
    vec3 ul = vec3(aabb.min.x, aabb.max.y, 0.0);
    vec3 lr = vec3(aabb.max.x, aabb.min.y, 0.0);
    vec3 ur = vec3(aabb.max.x, aabb.max.y, 0.0);
    push_debug_line(ll, lr, min_color);
    push_debug_line(lr, ur, max_color);
    push_debug_line(ur, ul, max_color);
    push_debug_line(ul, ll, min_color);
}

/// Pushes a diagonal cross in ndc on the xy plane with z = 0.0 to the debug line buffer.
void push_debug_ndc_cross(vec3 position, float size, vec4 color) {
    push_debug_line(position + vec3(-size, -size, 0.0), position + vec3(size, size, 0.0), color);
    push_debug_line(position + vec3(-size, size, 0.0), position + vec3(size, -size, 0.0), color);
}

/// Returns the view projection matrix of the active camera or the identity matrix if there is no active camera.
mat4 active_camera_view_projection_matrix() {
    if (per_frame_data.active_camera_instance >= 0) {
        CameraInstance camera_instance = camera_instances[per_frame_data.active_camera_instance];
        Camera camera = cameras[uint(camera_instance.camera_index)];
        return camera.projection_matrix * camera_instance.view_matrix;
    }
    return mat4(1.0);
}

/// Returns the view matrix of the active camera or the identity matrix if there is no active camera.
mat4 active_camera_view_matrix() {
    if (per_frame_data.active_camera_instance >= 0) {
        CameraInstance camera_instance = camera_instances[per_frame_data.active_camera_instance];
        return camera_instance.view_matrix;
    }
    return mat4(1.0);
}

/// Returns the projection matrix of the active camera or the identity matrix if there is no active camera.
mat4 active_camera_projection_matrix() {
    if (per_frame_data.active_camera_instance >= 0) {
        CameraInstance camera_instance = camera_instances[per_frame_data.active_camera_instance];
        Camera camera = cameras[uint(camera_instance.camera_index)];
        return camera.projection_matrix;
    }
    return mat4(1.0);
}

/// Returns the near plane of the active camera or -1.0 if there is no active camera.
float active_camera_znear() {
    if (per_frame_data.active_camera_instance >= 0) {
        CameraInstance camera_instance = camera_instances[per_frame_data.active_camera_instance];
        Camera camera = cameras[uint(camera_instance.camera_index)];
        return camera.znear;
    }
    return -1.0;
}





layout (push_constant) uniform PushConstants {
    uint _non_zero;
} push_constants;

layout (location = 0) out vec4 out_color;

void main() {
    const uint C = DEVICE_LOCAL_DEBUG_LINES_COMPONENTS_PER_LINE;

    bool is_start_vertex = gl_VertexIndex % 2 == 0;
    uint line_index = is_start_vertex ? gl_VertexIndex / 2 : (gl_VertexIndex - 1) / 2;

    // Set vertex position
    if (is_start_vertex) {
        gl_Position = vec4(
            device_local_debug_lines.lines[C * line_index + 0],
            device_local_debug_lines.lines[C * line_index + 1],
            device_local_debug_lines.lines[C * line_index + 2],
            1
        );
    } else {
        gl_Position = vec4(
            device_local_debug_lines.lines[C * line_index + 3],
            device_local_debug_lines.lines[C * line_index + 4],
            device_local_debug_lines.lines[C * line_index + 5],
            1
        );
    }

    // Set vertex color
    out_color = vec4(
        device_local_debug_lines.lines[C * line_index + 6],
        device_local_debug_lines.lines[C * line_index + 7],
        device_local_debug_lines.lines[C * line_index + 8],
        device_local_debug_lines.lines[C * line_index + 9]
    );
}