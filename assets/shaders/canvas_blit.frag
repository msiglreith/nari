#version 450 core

#extension GL_EXT_nonuniform_qualifier : require

#include "colorspace.h"

layout(push_constant) uniform Params {
	uint u_source;
};

layout (location = 0) in vec2 a_uv;
layout (location = 0) out vec4 o_color;

layout (set = 0, binding = 0) uniform sampler2D u_samplers[];

void main(void) {
    vec4 color = texture(u_samplers[nonuniformEXT(u_source)], a_uv);
    vec4 color_oklab = vec4(oklab_to_linear(color.rgb), color.a);
    o_color = color_oklab;
}
