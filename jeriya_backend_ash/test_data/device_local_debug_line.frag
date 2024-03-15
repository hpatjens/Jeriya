#version 450

layout (location = 0) in vec4 in_color;

layout (location = 0) out vec4 output_color;

layout (push_constant) uniform PushConstants {
    uint _non_zero;
} push_constants;

void main() {
    output_color = in_color;
}