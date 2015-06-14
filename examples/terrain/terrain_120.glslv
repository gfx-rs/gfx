#version 120

attribute vec3 a_Pos;
attribute vec3 a_Color;
varying vec3 v_Color;

uniform mat4 u_Model;
uniform mat4 u_View;
uniform mat4 u_Proj;

void main() {
    v_Color = a_Color;
    gl_Position = u_Proj * u_View * u_Model * vec4(a_Pos, 1.0);
}
