#include <metal_stdlib>
#include <metal_pack>

using namespace metal;

struct VertexInput {
    float2 a_Position   [[ attribute(0) ]];
    float2 a_Translate  [[ attribute(1) ]];
    uint   a_Color      [[ attribute(2) ]];
};

struct VertexOut {
    float4 pos [[ position ]];
    float4 color;
};

vertex VertexOut vert(VertexInput     in     [[ stage_in ]],
                      constant float& Locals [[ buffer(2) ]])
{
    VertexOut out;

    out.pos = float4((in.a_Position * Locals) + in.a_Translate, 0.0, 1.0);
    out.color = float4(in.a_Color >> 24,
                      (in.a_Color >> 16) & 0x000000FF,
                      (in.a_Color >> 8)  & 0x000000FF,
                       in.a_Color        & 0x000000FF) / 255.0;

    return out;
}


