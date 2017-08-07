#include <metal_stdlib>

using namespace metal;

struct VertexOut {
    float4 pos [[ position ]];
    float2 coords;
};

struct FragmentOut {
    float4 main [[ color(0) ]];
};

fragment FragmentOut frag(VertexOut in             [[ stage_in ]],
                     texture2d<float> t_Color [[ texture(0) ]],
                     sampler t_Color_         [[ sampler(0) ]])
{
    FragmentOut out;

    float4 t = t_Color.sample(t_Color_, in.coords);
    float blend = dot(in.coords - float2(0.5, 0.5), in.coords - float2(0.5, 0.5));
    out.main = mix(t, float4(0.0, 0.0, 0.0, 0.0), blend * 1.0);

    return out;
};

