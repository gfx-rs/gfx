#version 450
#extension GL_ARB_separate_shader_objects : enable
#extension GL_EXT_multiview : enable

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 target0;

layout(set = 0, binding = 0) uniform texture2D u_texture;
layout(set = 0, binding = 1) uniform sampler u_sampler;

void main() {
    vec4 mask = vec4(0, 0, 0, 1);

    mask[gl_ViewIndex] = 1.0;

    target0 = texture(sampler2D(u_texture, u_sampler), v_uv) * mask;
}
