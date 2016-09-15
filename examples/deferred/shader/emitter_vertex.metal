#include <metal_stdlib>

using namespace metal;

struct VertexInput {
    int3 a_Pos [[ attribute(0) ]];
};

constant const int NUM_LIGHTS = 250;

struct Cube {
    float4x4 transform;
    float radius;
};

struct Lights {
    float4 lights[NUM_LIGHTS];
};

vertex float4 vert(constant Cube&   CubeLocals    [[ buffer(1) ]],
                   constant Lights& LightPosBlock [[ buffer(2) ]],
                   VertexInput      in            [[ stage_in ]],
                   uint             iid           [[ instance_id ]])
{
    return CubeLocals.transform * float4(CubeLocals.radius * float3(in.a_Pos) + LightPosBlock.lights[iid].xyz, 1.0);
}
