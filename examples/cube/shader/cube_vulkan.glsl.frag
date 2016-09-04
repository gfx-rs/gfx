#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 v_TexCoord;
layout(location = 0) out vec4 Target0;
layout(set = 0, binding = 0) uniform sampler2D t_Color;

void main() {
    vec4 tex = texture(t_Color, v_TexCoord);
    float blend = dot(v_TexCoord-vec2(0.5,0.5), v_TexCoord-vec2(0.5,0.5));
    Target0 = mix(tex, vec4(0.0,0.0,0.0,0.0), blend*1.0);
}
