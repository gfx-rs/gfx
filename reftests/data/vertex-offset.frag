#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in flat uvec4 v_color;
layout(location = 0) out uvec4 o_Color;

void main() {
    o_Color = v_color;
}
