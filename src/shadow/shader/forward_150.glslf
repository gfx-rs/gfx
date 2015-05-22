#version 150 core

out vec4 o_Color;
uniform vec4 u_Color;

void main() {
    o_Color = u_Color;
}
