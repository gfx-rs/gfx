#include <metal_stdlib>
using namespace metal;

typedef struct {
    float4 src_coords [[attribute(0)]];
    float4 dst_coords [[attribute(1)]];
} TextureBlitAttributes;

typedef struct {
    float4 position [[position]];
    float4 uv;
    uint layer [[render_target_array_index]];
} VertexData;

vertex VertexData vs_blit(TextureBlitAttributes in [[stage_in]]) {
    float4 pos = { 0.0, 0.0, 0.0f, 1.0f };
    pos.xy = in.dst_coords.xy * 2.0 - 1.0;
    return VertexData { pos, in.src_coords, uint(in.dst_coords.z) };
}

fragment float4 ps_blit(
    VertexData in [[stage_in]],
    texture2d_array<float> tex [[ texture(0) ]],
    sampler sampler2D [[ sampler(0) ]]
) {
  return tex.sample(sampler2D, in.uv.xy, uint(in.uv.z), level(in.uv.w));
}
