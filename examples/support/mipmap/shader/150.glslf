#version 150

uniform sampler2D t_Tex;
in vec2 v_Uv;
out vec4 Target0;

void main() {
    vec3 color = texture(t_Tex, v_Uv).rgb;
    Target0 = vec4(color, 1.0);
}
