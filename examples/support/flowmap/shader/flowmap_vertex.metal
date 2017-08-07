#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    float2 a_Pos [[ attribute(0) ]];
    float2 a_Uv  [[ attribute(1) ]];
};

struct VertexOut {
    float4 pos [[ position ]];
    float2 coords;
};

vertex VertexOut vert(VertexInput in [[ stage_in ]])
{
    VertexOut out;

    out.pos = float4(in.a_Pos, 0.0, 1.0);
    out.coords = in.a_Uv;

    return out;
}

