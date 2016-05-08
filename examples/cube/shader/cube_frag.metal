#include <metal_stdlib>

using namespace metal;

typedef struct {
    float4 pos [[ position ]];
    float2 coords;
} VertexOut;

typedef struct {
    float4 main [[ color(0) ]];
} FragmentOut;

fragment FragmentOut frag(VertexOut in             [[ stage_in ]],
                          texture2d<float> t_Color [[ texture(0) ]],
                          sampler sampler          [[ sampler(0) ]])
{
    FragmentOut out;

    float4 t = t_Color.sample(sampler, in.coords);
    float blend = dot(in.coords - float2(0.5, 0.5), in.coords - float2(0.5, 0.5));
    out.main = mix(t, float4(0.0, 0.0, 0.0, 0.0), blend * 1.0);

    return out;
};

