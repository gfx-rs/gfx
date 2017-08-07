#version 120

attribute vec4 a_Pos;
attribute vec2 a_TexCoord;
varying vec2 v_TexCoord;

uniform mat4 u_Transform;

void main() {
    v_TexCoord = a_TexCoord;
    gl_Position = u_Transform * a_Pos;
}
