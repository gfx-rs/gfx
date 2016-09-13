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
    out.color = unpack_unorm4x8_to_float(in.a_Color);

    return out;
}


