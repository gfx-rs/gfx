#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    float3 a_Pos   [[ attribute(0) ]];
    float3 a_Color [[ attribute(1) ]];
};

struct VertexOut {
    float4 pos [[ position ]];
    float3 color;
};

struct VsLocals {
    float4x4 u_Model;
    float4x4 u_View;
    float4x4 u_Proj;
};

vertex VertexOut vert(constant VsLocals &Locals [[ buffer(1) ]],
                      VertexInput in            [[ stage_in ]])
{
    VertexOut out;

    out.pos = Locals.u_Proj * Locals.u_View * Locals.u_Model * float4(in.a_Pos, 1.0);
    out.color = in.a_Color;

    return out;
}

