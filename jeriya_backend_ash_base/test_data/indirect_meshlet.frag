#version 450

layout (constant_id = 0) const uint MAX_CAMERAS = 8;
layout (constant_id = 1) const uint MAX_INANIMATE_MESH_INSTANCES = 1024;

layout (location = 0) out vec4 outputColor;

layout (push_constant) uniform PushConstants {
    uint _non_zero;
} push_constants;

layout (location = 0) in vec3 in_vertex_normal;
layout (location = 1) flat in uint in_meshlet_index;

const vec3 COLORS[] = {
    vec3(0.9020, 0.9725, 0.0000),
    vec3(0.8314, 0.9843, 0.0196),
    vec3(0.7490, 0.9647, 0.0627),
    vec3(0.6784, 0.9490, 0.1098),
    vec3(0.6157, 0.9373, 0.1529),
    vec3(0.5647, 0.9216, 0.1922),
    vec3(0.5216, 0.9098, 0.2353),
    vec3(0.4863, 0.8980, 0.2745),
    vec3(0.4627, 0.8863, 0.3137),
    vec3(0.4431, 0.8784, 0.3529),
    vec3(0.4431, 0.8784, 0.3529),
    vec3(0.3373, 0.8863, 0.2941),
    vec3(0.2314, 0.8902, 0.2431),
    vec3(0.1686, 0.9020, 0.2549),
    vec3(0.1020, 0.9137, 0.2745),
    vec3(0.0667, 0.8941, 0.3216),
    vec3(0.0471, 0.8588, 0.3765),
    vec3(0.0314, 0.8235, 0.4275),
    vec3(0.0157, 0.7843, 0.4745),
    vec3(0.0000, 0.7451, 0.5176),
    vec3(0.0000, 0.7451, 0.5176),
    vec3(0.0000, 0.7294, 0.5294),
    vec3(0.0000, 0.7098, 0.5373),
    vec3(0.0000, 0.6941, 0.5490),
    vec3(0.0000, 0.6784, 0.5569),
    vec3(0.0000, 0.6588, 0.5647),
    vec3(0.0000, 0.6431, 0.5686),
    vec3(0.0000, 0.6275, 0.5725),
    vec3(0.0000, 0.6078, 0.5765),
    vec3(0.0000, 0.5922, 0.5804),
    vec3(0.0000, 0.5922, 0.5804),
    vec3(0.0000, 0.5843, 0.5843),
    vec3(0.0000, 0.5647, 0.5765),
    vec3(0.0000, 0.5451, 0.5647),
    vec3(0.0000, 0.5255, 0.5569),
    vec3(0.0000, 0.5059, 0.5490),
    vec3(0.0000, 0.4902, 0.5412),
    vec3(0.0000, 0.4706, 0.5294),
    vec3(0.0000, 0.4510, 0.5216),
    vec3(0.0000, 0.4353, 0.5137),
    vec3(0.0000, 0.4353, 0.5137),
    vec3(0.0196, 0.4118, 0.4941),
    vec3(0.0392, 0.3882, 0.4784),
    vec3(0.0588, 0.3647, 0.4588),
    vec3(0.0824, 0.3490, 0.4392),
    vec3(0.1020, 0.3294, 0.4196),
    vec3(0.1216, 0.3137, 0.4039),
    vec3(0.1412, 0.3020, 0.3843),
    vec3(0.1647, 0.2902, 0.3647),
    vec3(0.1843, 0.2824, 0.3451),
};

uint esgtsa(uint s) {
    s = (s ^ 2747636419u) * 2654435769u;// % 4294967296u;
    s = (s ^ (s >> 16u)) * 2654435769u;// % 4294967296u;
    s = (s ^ (s >> 16u)) * 2654435769u;// % 4294967296u;
    return s;
}

void main() {
    vec3 color = COLORS[esgtsa(in_meshlet_index) % 50];
    outputColor = vec4(color, 1.0);
}