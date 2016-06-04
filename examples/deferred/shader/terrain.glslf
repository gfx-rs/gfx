#version 150 core

in vec3 v_FragPos;
in vec3 v_Normal;
in vec3 v_Color;
out vec4 Target0;
out vec4 Target1;
out vec4 Target2;

void main() {
	vec3 n = normalize(v_Normal);

	Target0 = vec4(v_FragPos, 0.0);
	Target1 = vec4(n, 0.0);
	Target2 = vec4(v_Color, 1.0);
}
