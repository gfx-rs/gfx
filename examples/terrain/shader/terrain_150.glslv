#version 150 core

in vec3 a_Pos;
in vec3 a_Color;
out vec3 v_Color;

layout (std140)
uniform Locals {
	mat4 u_Model;
	mat4 u_View;
	mat4 u_Proj;
};

void main() {
    v_Color = a_Color;
    gl_Position = u_Proj * u_View * u_Model * vec4(a_Pos, 1.0);
    gl_ClipDistance[0] = 1.0;
}
