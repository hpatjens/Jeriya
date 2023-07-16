#version 450

#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require

layout (constant_id = 0) const uint MAX_CAMERAS = 8;
layout (constant_id = 1) const uint MAX_INANIMATE_MESh_INSTANCES = 1024;

layout (location = 0) in vec3 inPosition;

layout (push_constant) uniform PushConstants {
    vec4 color;
    mat4 matrix;
} push_constants;

struct Camera {
    mat4 projection_matrix;
    mat4 view_matrix;
    mat4 matrix;
};

struct InanimateMeshInstance {
    uint64_t inanimate_mesh_id;
    mat4 transform;
};

layout (set = 0, binding = 0) uniform PerFrameData { uint active_camera; } per_frame_data;
layout (set = 0, binding = 1) buffer Cameras { Camera cameras[MAX_CAMERAS]; } cameras;
layout (set = 0, binding = 2) buffer InanimateMeshInstances { InanimateMeshInstance inanimate_mesh_instances[MAX_INANIMATE_MESh_INSTANCES]; } inanimate_mesh_instances;

void main() {
    mat4 matrix = cameras.cameras[per_frame_data.active_camera].matrix;
    gl_Position = matrix * vec4(inPosition, 1.0);
}