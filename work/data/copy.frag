#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(set = 0, binding = 0) uniform texture2D t_Color;
layout(set = 0, binding = 1) uniform sampler s_Color;

layout(location = 0) in vec2 v_TexCoords;
layout(location = 0) out vec4 o_Color;

void main() {
    o_Color = texture(sampler2D(t_Color, s_Color), v_TexCoords);
}
