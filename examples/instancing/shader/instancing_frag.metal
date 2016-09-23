#include <metal_stdlib>

using namespace metal;

struct VertexOut {
    float4 pos [[ position ]];
    float4 color;
};

struct FragmentOut {
    float4 main [[ color(0) ]];
};

fragment FragmentOut frag(VertexOut in [[ stage_in ]])
{
    FragmentOut out;

    out.main = in.color;

    return out;
};

