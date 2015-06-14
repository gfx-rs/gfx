#version 150 core

in vec3 a_Pos;

uniform mat4 u_Transform;

void main() {
    gl_Position = u_Transform * vec4(a_Pos, 1.0);
}
