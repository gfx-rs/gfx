#include <metal_stdlib>

using namespace metal;

struct VertexOut {
    float4 pos [[ position ]];
    float2 uv;
};

struct FragmentOut {
    float4 main [[ color(0) ]];
};

fragment FragmentOut frag(VertexOut in [[ stage_in ]],
                          texture2d<float, access::sample> t_BlitTex [[ texture(0) ]],
                          sampler t_BlitTex_                         [[ sampler(0) ]])
{
    FragmentOut out;

    out.main = t_BlitTex.sample(t_BlitTex_, in.uv);

    return out;
}
