#version 450

layout (constant_id = 0) const uint MAX_CAMERAS = 8;
layout (constant_id = 1) const uint MAX_INANIMATE_MESH_INSTANCES = 1024;

layout (location = 0) out vec4 outputColor;

layout (push_constant) uniform PushConstants {
    vec4 color;
    mat4 matrix;
} push_constants;

void main() {
    outputColor = push_constants.color;
}