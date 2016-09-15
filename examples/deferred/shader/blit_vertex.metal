#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    int4 a_PosTexCoord [[ attribute(0) ]];
};

struct VertexOut {
    float4 pos [[ position ]];
    float2 uv;
};

vertex VertexOut vert(VertexInput in [[ stage_in ]])
{
    VertexOut out;

    out.pos = float4(float2(in.a_PosTexCoord.xy), 0.0, 1.0);

    out.uv = float2(in.a_PosTexCoord.zw);
    out.uv.y = 1.0 - out.uv.y;

    return out;
}
