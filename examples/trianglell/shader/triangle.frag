#version 450
#extension GL_ARB_separate_shader_objects : enable

in vec4 v_Color;
out vec4 Target0;

void main() {
    Target0 = v_Color;
}
