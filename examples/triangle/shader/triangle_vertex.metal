#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    float4 a_Pos       [[ attribute(0) ]];
    float3 a_Color     [[ attribute(1) ]];
};

struct VertexOut {
    float4 pos [[ position ]];
    float3 color;
};

vertex VertexOut vert(uint vid                  [[ vertex_id ]],
                      VertexInput in            [[ stage_in ]])
{
    VertexOut out;

    out.pos = in.a_Pos;
    out.color = in.a_Color;

    return out;
}

