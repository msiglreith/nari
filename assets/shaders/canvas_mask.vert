#version 450 core
#extension GL_EXT_buffer_reference : enable

#include "colorspace.h"

struct Vertex {
	ivec2 pos;
    uint page;
    uint tile;
    vec4 color;
};

layout(buffer_reference, std430) buffer Vertices {
    Vertex vertex[];
};

layout(push_constant) uniform Params {
	Vertices vertices;
    ivec2 offset;
    ivec2 extent;
};

layout (location = 0) out uint a_page;
layout (location = 1) out vec2 a_tile;
layout (location = 2) out vec4 a_color;

void main() {
    Vertex vertex = vertices.vertex[gl_VertexIndex];

    vec3 c_linear = srgb_to_linear(vertex.color.rgb);
    vec3 c_oklab = linear_to_oklab(c_linear);

    a_color = vec4(c_oklab, vertex.color.a);
    a_page = vertex.page;
    a_tile = vec2(float(vertex.tile >> 16), float(vertex.tile & 0xFFFF));
    gl_Position = vec4(vec2(2 * (vertex.pos - offset) - extent) / vec2(extent), 0.0, 1.0);
}
