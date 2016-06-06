#version 150 core

in ivec2 a_Pos;
in ivec2 a_TexCoord;
out vec2 v_TexCoord;

void main() {
	v_TexCoord = a_TexCoord;
	gl_Position = vec4(a_Pos, 0.0, 1.0);
}
