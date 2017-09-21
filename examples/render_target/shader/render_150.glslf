#version 150 core

in vec4 FragPos;

uniform Locals {
	vec3 pos;
	float farPlane;
};

void main() {
	gl_FragDepth = 0.0;
}
