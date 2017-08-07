#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    int4 a_Pos [[ attribute(0) ]];
};

vertex float4 vert(constant float4x4& Locals [[ buffer(1) ]],
                   VertexInput in            [[ stage_in ]])
{
    float4 ndc = Locals * float4(in.a_Pos);
    ndc.z = (ndc.z + ndc.w) * 0.5;
    return ndc;
}

