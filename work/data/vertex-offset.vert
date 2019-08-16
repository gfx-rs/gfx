#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in uvec4 a_color;
layout(location = 0) out flat uvec4 v_color;

void main() {
    vec2 pos = vec2(0.0);
    if (gl_VertexIndex==0) pos = vec2(-1.0, -3.0);
    if (gl_VertexIndex==1) pos = vec2(3.0, 1.0);
    if (gl_VertexIndex==2) pos = vec2(-1.0, 1.0);
    gl_Position = vec4(pos, 0.0, 1.0);
    v_color = a_color;
}
