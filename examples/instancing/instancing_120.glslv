#version 130

attribute vec2 a_Position;
attribute vec2 a_Translate;
attribute uint a_Color;

uniform float u_Scale;

varying vec4 v_Color;

void main() {
    gl_Position = vec4((a_Position*u_Scale) + a_Translate, 0.0, 1.0);

    uint u8mask = 0x000000FFu;
    v_Color = vec4(float( a_Color >> 24),
                   float((a_Color >> 16) & u8mask),
                   float((a_Color >>  8) & u8mask),
                   float( a_Color        & u8mask)) / 255.0;
}
