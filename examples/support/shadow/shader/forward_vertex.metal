#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    int4 a_Pos    [[ attribute(0) ]];
    int4 a_Normal [[ attribute(1) ]];
};

struct VertexOut {
    float4 pos [[ position ]];
    float3 position;
    float3 normal;
};

struct Locals {
    float4x4 u_Transform;
    float4x4 u_ModelTransform;
};

vertex VertexOut vert(constant Locals &VsLocals [[ buffer(1) ]],
                      VertexInput in            [[ stage_in ]])
{
    VertexOut out;

    out.pos = VsLocals.u_Transform * float4(in.a_Pos);

    out.position = (VsLocals.u_ModelTransform * float4(in.a_Pos)).xyz;
    out.normal = (VsLocals.u_ModelTransform * float4(in.a_Normal)).xyz;

    return out;
}


