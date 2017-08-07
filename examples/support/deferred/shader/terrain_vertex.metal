#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    float3 a_Pos    [[ attribute(0) ]];
    float3 a_Normal [[ attribute(1) ]];
    float3 a_Color  [[ attribute(2) ]];
};

struct VertexOut {
    float4 pos [[ position ]];
    float3 position;
    float3 normal;
    float3 color;
};

struct VsLocals {
    float4x4 model;
    float4x4 view;
    float4x4 proj;
};

vertex VertexOut vert(constant VsLocals &TerrainLocals [[ buffer(1) ]],
                      VertexInput in            [[ stage_in ]])
{
    VertexOut out;

    out.pos = TerrainLocals.proj * TerrainLocals.view * TerrainLocals.model * float4(in.a_Pos, 1.0);
    out.position = (TerrainLocals.model * float4(in.a_Pos, 1.0)).xyz;
    out.normal = (TerrainLocals.model * float4(in.a_Normal, 1.0)).xyz;
    out.color = in.a_Color;

    return out;
}
