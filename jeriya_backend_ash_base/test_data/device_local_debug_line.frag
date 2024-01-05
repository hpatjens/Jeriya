#version 450

layout (location = 0) out vec4 outputColor;

layout (push_constant) uniform PushConstants {
    uint _non_zero;
} push_constants;

void main() {
    outputColor = vec4(1.0, 0.0, 0.0, 1.0);
}