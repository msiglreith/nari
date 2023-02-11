#version 450 core

layout (location = 0) out vec2 a_uv;

void main() {
    a_uv = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
	gl_Position = vec4(2 * a_uv - 1, 0.0, 1.0);
}
