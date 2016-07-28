#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    int4 a_Pos [[ attribute(0) ]];
};

vertex float4 vert(constant float4x4& Locals [[ buffer(1) ]],
                   VertexInput in            [[ stage_in ]])
{
    return Locals * float4(in.a_Pos);
}

