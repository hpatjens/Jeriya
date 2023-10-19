#version 450

#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require
#extension GL_ARB_shader_draw_parameters : enable

layout (constant_id = 0) const uint MAX_CAMERAS = 16;
layout (constant_id = 1) const uint MAX_CAMERA_INSTANCES = 64;
layout (constant_id = 3) const uint MAX_RIGID_MESHES = 1024;
layout (constant_id = 4) const uint MAX_MESH_ATTRIBUTES = 1024;
layout (constant_id = 5) const uint MAX_RIGID_MESH_INSTANCES = 1024;
layout (constant_id = 6) const uint MAX_MESHLETS = 1024;
layout (constant_id = 7) const uint MAX_VISIBLE_RIGID_MESH_INSTANCES = 1024;
layout (constant_id = 8) const uint MAX_VISIBLE_RIGID_MESH_MESHLETS = 1024;

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

struct RigidMesh {
    int mesh_attributes_index;
    uint preferred_mesh_representation;
};

struct RigidMeshInstance {
    uint64_t rigid_mesh_index;
    mat4 transform;
};

layout (set = 0, binding = 0) uniform PerFrameData { 
    int active_camera_instance; // -1 means no active camera
    uint mesh_attributes_count;
    uint rigid_mesh_count;
    uint rigid_mesh_instance_count;
} per_frame_data;

layout (set = 0, binding = 1) buffer Cameras { 
    Camera cameras[MAX_CAMERAS];
};

layout (set = 0, binding = 2) buffer CameraInstanceBuffer { 
    CameraInstance camera_instances[MAX_CAMERA_INSTANCES];
};

layout (set = 0, binding = 3) buffer IndirectDrawRigidMeshInstanceBuffer { 
    VkDrawIndirectCommand indirect_draw_rigid_mesh_instances[MAX_RIGID_MESH_INSTANCES];
};

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
} visible_rigid_mesh_meshlets;





layout (push_constant) uniform PushConstants {
    uint _non_zero;
} push_constants;

layout (location = 0) out vec3 out_vertex_normal;

void main() {
    RigidMeshInstance rigid_mesh_instance = rigid_mesh_instances[gl_DrawIDARB];
    RigidMesh rigid_mesh = rigid_meshes[uint(rigid_mesh_instance.rigid_mesh_index)];
    MeshAttributes mesh_attributes = mesh_attributes[uint(rigid_mesh.mesh_attributes_index)];
    bool mesh_attributes_active = mesh_attributes_active[uint(rigid_mesh.mesh_attributes_index)];

    // MeshAttributes become active when the transfer to the GPU is complete. When the transfer is
    // not yet complete, the RigidMeshInstance cannot be rendered.
    if (!mesh_attributes_active) {
        return;
    }

    mat4 view_projection_matrix;
    if (per_frame_data.active_camera_instance >= 0) {
        CameraInstance camera_instance = camera_instances[per_frame_data.active_camera_instance];
        Camera camera = cameras[uint(camera_instance.camera_index)];
        view_projection_matrix = camera.projection_matrix * camera_instance.view_matrix;
    } else {
        view_projection_matrix = mat4(1.0);
    }

    mat4 model_matrix = rigid_mesh_instance.transform;
    mat4 matrix = view_projection_matrix * model_matrix;

    vec3 vertex_position;
    vec3 vertex_normal;
    // When the attributes don't contain indices, the `indices_len` is set to 0.
    if (mesh_attributes.indices_len > 0) {
        // In this case, the shader invocation runs per index of the mesh and the
        // corresponding vertex attribute has to be looked up via the index buffer.
        uint index_index = uint(mesh_attributes.indices_start_offset) + gl_VertexIndex;
        uint attribute_index = indices[index_index];
        uint offset = uint(mesh_attributes.vertex_positions_start_offset);
        vertex_position = vertex_positions[offset + attribute_index].xyz;
        vertex_normal = vertex_normals[offset + attribute_index].xyz;
    } else {
        // In this case, the shader invocation runs per vertex of the mesh directly.
        uint64_t attribute_index = mesh_attributes.vertex_positions_start_offset + gl_VertexIndex;
        vertex_position = vertex_positions[uint(attribute_index)].xyz;
        vertex_normal = vertex_normals[uint(attribute_index)].xyz;
    }

    out_vertex_normal = vertex_normal;
    gl_Position = matrix * vec4(vertex_position, 1.0);
}