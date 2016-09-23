#include <metal_stdlib>

using namespace metal;

struct VertexOut {
    float4 pos [[ position ]];
    float3 position;
    float3 normal;
    float3 color;
};

struct FragmentOut {
    float4 pos    [[ color(0) ]];
    float4 normal [[ color(1) ]];
    float4 color  [[ color(2) ]];
};

fragment FragmentOut frag(VertexOut in [[ stage_in ]]) {
    FragmentOut out;

    float3 n = normalize(in.normal);

    out.pos = float4(in.position, 0.0);
    out.normal = float4(n, 0.0);
    out.color = float4(in.color, 1.0);

    return out;
};

