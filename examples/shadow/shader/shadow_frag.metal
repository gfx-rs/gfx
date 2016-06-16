#include <metal_stdlib>

using namespace metal;

struct VertexOut {
    float4 pos [[ position ]];
    float2 coords;
};

struct FragmentOut {
    float4 main [[ color(0) ]];
};

fragment FragmentOut frag(constant float2& Locals  [[ buffer(1) ]],
                          VertexOut in             [[ stage_in ]],
                          texture2d<float> t_Color [[ texture(0) ]],
                          texture2d<float> t_Flow  [[ texture(1) ]],
                          texture2d<float> t_Noise [[ texture(2) ]],
                          sampler t_Color_         [[ sampler(0) ]],
                          sampler t_Flow_          [[ sampler(1) ]],
                          sampler t_Noise_         [[ sampler(2) ]])
{
    FragmentOut out;

    // we sample the direction from our flow map, then map it to a [-1, 1] range
    float2 flow = t_Flow.sample(t_Flow_, in.coords).xy * 2.0 - 1.0;

    // we apply some noise to get rid of the visible repeat pattern
    float noise = t_Noise.sample(t_Noise_, in.coords).r;

    // apply the noise to our cycles
    float phase0 = noise * .05f + Locals.x * .25f;
    float phase1 = noise * .05f + Locals.y * .25f;

    // grab two samples to interpolate between
    float3 t0 = t_Color.sample(t_Color_, in.coords + flow * phase0).xyz;
    float3 t1 = t_Color.sample(t_Color_, in.coords + flow * phase1).xyz;

    float lerp = 2.0 * abs(Locals.x - .5f);
    float3 result = mix(t0, t1, lerp);

    out.main = float4(result, 1.0);

    return out;
};

