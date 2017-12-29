#version 300 es
precision mediump float;

in vec3 v_Color;
out vec4 Target0;

void main() {
    Target0 = vec4(v_Color, 1.0);
}
