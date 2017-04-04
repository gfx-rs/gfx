#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 a_Pos;
layout(location = 1) in vec3 a_Color;
layout(location = 0) out vec4 v_Color;

void main() {
    v_Color = vec4(a_Color, 1.0);
    gl_Position = vec4(a_Pos, 0.0, 1.0);
}
