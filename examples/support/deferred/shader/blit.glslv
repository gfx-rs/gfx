#version 150 core

in ivec4 a_PosTexCoord;
out vec2 v_TexCoord;

void main() {
	v_TexCoord = a_PosTexCoord.zw;
	gl_Position = vec4(a_PosTexCoord.xy, 0.0, 1.0);
}
