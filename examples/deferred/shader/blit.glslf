#version 150 core

uniform sampler2D t_BlitTex;
in vec2 v_TexCoord;
out vec4 Target0;

void main() {
	vec4 tex = texture(t_BlitTex, v_TexCoord);
	Target0 = tex;
}
