#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    float4 a_Pos       [[ attribute(0) ]];
    float2 a_TexCoord  [[ attribute(1) ]];
};

struct VertexOut {
    float4 pos [[ position ]];
    float2 coords;
};

vertex VertexOut vert(constant float4x4 &Locals [[ buffer(1) ]],
                      uint vid                  [[ vertex_id ]],
                      VertexInput in            [[ stage_in ]])
{
    VertexOut out;

    out.pos = Locals * in.a_Pos;
    out.coords = in.a_TexCoord;

    return out;
}

