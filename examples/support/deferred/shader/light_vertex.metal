#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    int3 a_Pos [[ attribute(0) ]];
};

struct VertexOut {
    float4 pos [[ position ]];
    float3 light_pos;
};

constant const int NUM_LIGHTS = 250;

struct Cube {
    float4x4 transform;
    float radius;
};

struct Lights {
    float4 lights[NUM_LIGHTS];
};

vertex VertexOut vert(constant Cube&   CubeLocals    [[ buffer(1) ]],
                      constant Lights& LightPosBlock [[ buffer(2) ]],
                      VertexInput      in            [[ stage_in ]],
                      uint             iid           [[ instance_id ]])
{
    VertexOut out;

    out.light_pos = LightPosBlock.lights[iid].xyz;
    out.pos = CubeLocals.transform * float4(CubeLocals.radius * float3(in.a_Pos) + out.light_pos, 1.0);

    return out;
}
