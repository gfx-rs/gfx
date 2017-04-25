#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec4 v_Color;
layout(location = 0) out vec4 Target0;

layout(set = 0, binding = 0) uniform texture2D u_Texture;
layout(set = 1, binding = 0) uniform sampler u_Sampler;

void main() {
    Target0 = texture(sampler2D(u_Texture, u_Sampler), v_Color.xy);
}
