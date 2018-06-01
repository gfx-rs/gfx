#include <metal_stdlib>
using namespace metal;

typedef struct {
    float4 coords [[attribute(0)]];
} ClearAttributes;

typedef struct {
    float4 position [[position]];
    uint layer [[render_target_array_index]];
} ClearVertexData;

vertex ClearVertexData vs_clear(ClearAttributes in [[stage_in]]) {
    float4 pos = { 0.0, 0.0, 0.0f, 1.0f };
    pos.xy = in.coords.xy * 2.0 - 1.0;
    return ClearVertexData { pos, uint(in.coords.z) };
}


fragment float4 ps_clear_float(
    ClearVertexData in [[stage_in]],
    constant float4 &value [[ buffer(0) ]]
) {
  return value;
}

fragment int4 ps_clear_int(
    ClearVertexData in [[stage_in]],
    constant int4 &value [[ buffer(0) ]]
) {
  return value;
}

fragment uint4 ps_clear_uint(
    ClearVertexData in [[stage_in]],
    constant uint4 &value [[ buffer(0) ]]
) {
  return value;
}
