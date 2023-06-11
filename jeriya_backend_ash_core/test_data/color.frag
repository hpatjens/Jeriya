#version 450

layout (location = 0) out vec4 outputColor;

layout (push_constant) uniform constants
{
    vec4 color;
} PushConstants;

void main() {
    outputColor = PushConstants.color;
}