#version 450

void main() {
    switch (gl_VertexIndex) {
        case 0: 
            gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
            break;
        case 1: 
            gl_Position = vec4(1.0, 0.0, 0.0, 1.0);
            break;
        case 2:
            gl_Position = vec4(0.0, 1.0, 0.0, 1.0);
            break;
        default:
            gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
    }
}