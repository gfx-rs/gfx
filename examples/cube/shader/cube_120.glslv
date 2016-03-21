#version 120

attribute ivec4 a_Pos;
attribute ivec2 a_TexCoord;
varying vec2 v_TexCoord;

uniform mat4 u_Transform;

void main() {
    v_TexCoord = a_TexCoord;
    gl_Position = u_Transform * a_Pos;
}
