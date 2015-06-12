#version 150 core

in vec2 v_TexCoord;
out vec4 o_Color;
uniform sampler2D t_Color;

void main() {
    vec4 tex = texture(t_Color, v_TexCoord);
    float blend = dot(v_TexCoord-vec2(0.5,0.5), v_TexCoord-vec2(0.5,0.5));
    o_Color = mix(tex, vec4(0.0,0.0,0.0,0.0), blend*1.0);
}
