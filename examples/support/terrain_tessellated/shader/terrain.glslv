#version 150 core

in vec3 a_Pos;
in vec3 a_Color;

out block {
    vec3 pos;
	vec3 color;
} Out;

void main() {
    Out.pos = a_Pos;
    Out.color = a_Color;
}
