#version 120

uniform sampler2D t_Tex;
varying vec2 v_Uv;

void main() {
    vec3 color = texture2D(t_Tex, v_Uv).rgb;
    gl_FragColor = vec4(color, 1.0);
}
