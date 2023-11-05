#version 450

layout (constant_id = 0) const uint MAX_CAMERAS = 8;
layout (constant_id = 1) const uint MAX_INANIMATE_MESH_INSTANCES = 1024;

layout (location = 0) out vec4 outputColor;

layout (push_constant) uniform PushConstants {
    vec4 color;
    mat4 matrix;
} push_constants;

layout (location = 0) in vec4 in_point_color;
layout (location = 1) in vec2 in_texcoord;

void main() {
    const float extent_down = 0.288675;
    if (length(in_texcoord) > extent_down) {
        discard;
        return;
    }
    outputColor = in_point_color;
}