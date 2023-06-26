#version 450

layout (location = 0) in vec3 inPosition;

layout (push_constant) uniform PushConstants {
    vec4 color;
    mat4 matrix;
} push_constants;

layout (set = 0, binding = 0) uniform PerFrameData {
    mat4 projection_matrix;
    mat4 view_matrix;
    mat4 matrix;
} per_frame_data;

void main() {
    gl_Position = per_frame_data.matrix * vec4(inPosition, 1.0);
}