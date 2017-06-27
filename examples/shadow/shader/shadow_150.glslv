#version 150 core

in ivec4 a_Pos;

layout (std140)
uniform Locals {
	mat4 u_Transform;
};

void main() {
    gl_Position = u_Transform * vec4(a_Pos);
}
