#version 450

layout (constant_id = 0) const uint MAX_CAMERAS = 8;
layout (constant_id = 1) const uint MAX_INANIMATE_MESH_INSTANCES = 1024;

layout (location = 0) out vec4 outputColor;

layout (push_constant) uniform PushConstants {
    uint _non_zero;
} push_constants;

layout (location = 0) in vec3 in_vertex_normal;

void main() {
    outputColor = vec4(0.5 * in_vertex_normal + vec3(0.5), 1.0);
}