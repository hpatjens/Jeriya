#version 450

layout (constant_id = 0) const uint MAX_CAMERAS = 8;

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

layout (set = 0, binding = 0) uniform PerFrameData {
    mat4 projection_matrix;
    mat4 view_matrix;
    mat4 matrix;
    Camera cameras[MAX_CAMERAS];
} per_frame_data;

void main() {
    gl_Position = per_frame_data.matrix * vec4(inPosition, 1.0);
}