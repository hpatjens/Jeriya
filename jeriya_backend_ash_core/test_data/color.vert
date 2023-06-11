#version 450

layout (location = 0) in vec3 inPosition;

layout (push_constant) uniform constants
{
    vec4 color;
    mat4 matrix;
} PushConstants;

void main() {
    gl_Position = PushConstants.matrix * vec4(inPosition, 1.0);
}