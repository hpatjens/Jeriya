#version 450

layout (constant_id = 0) const uint MAX_CAMERAS = 8;

layout (location = 0) out vec4 outputColor;

layout (push_constant) uniform constants
{
    vec4 color;
    mat4 matrix;
} PushConstants;

void main() {
    outputColor = PushConstants.color;
}