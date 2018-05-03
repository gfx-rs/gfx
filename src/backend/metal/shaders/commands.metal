#include <metal_stdlib>
using namespace metal;

typedef struct {
    float2 src_coords [[attribute(0)]];
    float2 dst_coords [[attribute(1)]];
} TextureBlitAttributes;

typedef struct {
    float4 position [[position]];
    float2 tex_coords;
} VertexData;

vertex VertexData vs_blit(TextureBlitAttributes in [[stage_in]]) {
    float4 pos = { in.dst_coords.x, in.dst_coords.y, 0.0f, 1.0f };
    return VertexData { pos, in.src_coords };
}

fragment float4 ps_blit(
    VertexData in [[stage_in]],
    texture2d<float> tex2D [[ texture(0) ]],
    sampler sampler2D [[ sampler(0) ]]
) {
  return tex2D.sample(sampler2D, in.tex_coords);
}
