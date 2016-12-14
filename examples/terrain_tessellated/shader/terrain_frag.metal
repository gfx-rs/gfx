#include <metal_stdlib>

using namespace metal;

struct VertexOut {
    float4 pos [[ position ]];
    float3 color;
};

struct FragmentOut {
    float4 main [[ color(0) ]];
};

fragment FragmentOut frag(VertexOut in [[ stage_in ]]) {
    FragmentOut out;
    out.main = float4(in.color, 1.0);
    return out;
};


