#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(constant_id = 0) const float scale = 1.2f;

layout(location = 0) in vec2 a_Pos;
layout(location = 1) in vec2 a_Uv;
layout(location = 0) out vec2 v_Uv;

void main() {
    v_Uv = a_Uv;
    gl_Position = vec4(scale * a_Pos, 0.0, 1.0);
}
