#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 a_Pos;

void main() {
    gl_Position = vec4(a_Pos, 0.0, 1.0);
}
