#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    char4 a_Pos       [[ attribute(0) ]];
    char2 a_TexCoord  [[ attribute(1) ]];
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

    out.pos = Locals * float4(in.a_Pos);
    out.coords = float2(in.a_TexCoord);

    return out;
}

