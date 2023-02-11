#version 450 core

#extension GL_EXT_nonuniform_qualifier : require

#include "colorspace.h"

// eyeballing
#define CONTRAST 0.5
#define GAMMA 2.2

layout (location = 0) in flat uint a_page;
layout (location = 1) in vec2 a_tile;
layout (location = 2) in vec4 a_color;

layout (location = 0) out vec4 o_color;

layout (set = 0, binding = 0) uniform usampler2D u_samplers[];

float apply_contrast(float srca, float contrast) {
    return srca + ((1.0 - srca) * contrast * srca);
}

void main() {
    float mask = 1.0;
    if (a_page != ~0) {
        mask *= texelFetch(u_samplers[nonuniformEXT(a_page)], ivec2(a_tile), 0).r / 255.0;
    }

    // based on skia's gamma hack.
    float luma = a_color.r;

    float src = luma;
    float dst = 1.0 - luma;
    float lin_src = pow(src, GAMMA);
    float lin_dst = pow(dst, GAMMA);

    float adjusted_contrast = lin_dst * CONTRAST;

    float srca = apply_contrast(mask, adjusted_contrast);
    float dsta = 1.0 - srca;
    float lin_out = lin_src * srca + dsta * lin_dst;
    float c_out = pow(lin_out, 1.0 / GAMMA);
    mask = (c_out - dst) / (src - dst);

    mask *= a_color.a;
    o_color = vec4(a_color.rgb * mask, mask);
}
