#version 120

uniform sampler2D t_Color;
uniform sampler2D t_Flow;
uniform sampler2D t_Noise;

uniform float f_Offset0;
uniform float f_Offset1;

varying vec2 v_Uv;

void main() {
    // we sample the direction from our flow map, then map it to a [-1, 1] range
    vec2 flow = texture2D(t_Flow, v_Uv).rg * 2.0 - 1.0;

    // we apply some noise to get rid of the visible repeat pattern
    float noise = texture2D(t_Noise, v_Uv).r;
    
    // apply the noise to our cycles
    float phase0 = noise * .05f + f_Offset0 * .25f;
    float phase1 = noise * .05f + f_Offset1 * .25f;

    // grab two samples to interpolate between
    vec3 t0 = texture2D(t_Color, v_Uv + flow * phase0).rgb;
    vec3 t1 = texture2D(t_Color, v_Uv + flow * phase1).rgb;

    float lerp = 2.0 * abs(f_Offset0 - .5f);
    vec3 result = mix(t0, t1, lerp);

    gl_FragColor = vec4(result, 1.0);
}
