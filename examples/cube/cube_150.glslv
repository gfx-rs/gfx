#version 150 core

in vec3 a_Pos;
in vec2 a_TexCoord;
out vec2 v_TexCoord;

uniform mat4 u_Transform;

void main() {
    v_TexCoord = a_TexCoord;
    gl_Position = u_Transform * vec4(a_Pos, 1.0);
}
