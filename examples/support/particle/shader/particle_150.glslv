#version 150 core

in vec2 a_Pos;
in vec4 a_Color;

out VertexData {
    vec4 color;
} VertexOut;

void main() {
    gl_Position = vec4(a_Pos, 0, 1);
    VertexOut.color = a_Color;
}
