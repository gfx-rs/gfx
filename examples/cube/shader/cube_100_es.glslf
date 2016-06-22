#version 100

precision mediump float;
precision mediump int;

varying vec2 v_TexCoord;
uniform sampler2D t_Color;

void main() {
    vec4 tex = texture2D(t_Color, v_TexCoord);
    float blend = dot(v_TexCoord-vec2(0.5,0.5), v_TexCoord-vec2(0.5,0.5));
    gl_FragColor = mix(tex, vec4(0.0,0.0,0.0,0.0), blend*1.0);
}
