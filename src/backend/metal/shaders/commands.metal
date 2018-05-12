#include <metal_stdlib>
using namespace metal;

// -------------- Image Clears -------------- //

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

fragment float4 ps_blit_float(
    ClearVertexData in [[stage_in]],
    constant float4 &value [[ buffer(0) ]]
) {
  return value;
}

fragment int4 ps_blit_int(
    ClearVertexData in [[stage_in]],
    constant int4 &value [[ buffer(0) ]]
) {
  return value;
}

fragment uint4 ps_blit_uint(
    ClearVertexData in [[stage_in]],
    constant uint4 &value [[ buffer(0) ]]
) {
  return value;
}

// -------------- Image Blits -------------- //

typedef struct {
    float4 src_coords [[attribute(0)]];
    float4 dst_coords [[attribute(1)]];
} BlitAttributes;

typedef struct {
    float4 position [[position]];
    float4 uv;
    uint layer [[render_target_array_index]];
} BlitVertexData;

vertex BlitVertexData vs_blit(BlitAttributes in [[stage_in]]) {
    float4 pos = { 0.0, 0.0, 0.0f, 1.0f };
    pos.xy = in.dst_coords.xy * 2.0 - 1.0;
    return BlitVertexData { pos, in.src_coords, uint(in.dst_coords.z) };
}

fragment float4 ps_blit_1d_float(
    BlitVertexData in [[stage_in]],
    texture1d<float> tex1D [[ texture(0) ]],
    sampler sampler2D [[ sampler(0) ]]
) {
  return tex1D.sample(sampler2D, in.uv.x);
}

fragment float4 ps_blit_1d_array_float(
    BlitVertexData in [[stage_in]],
    texture1d_array<float> tex1DArray [[ texture(0) ]],
    sampler sampler2D [[ sampler(0) ]]
) {
  return tex1DArray.sample(sampler2D, in.uv.x, uint(in.uv.z));
}

fragment float4 ps_blit_2d_float(
    BlitVertexData in [[stage_in]],
    texture2d<float> tex2D [[ texture(0) ]],
    sampler sampler2D [[ sampler(0) ]]
) {
  return tex2D.sample(sampler2D, in.uv.xy, level(in.uv.w));
}

fragment uint4 ps_blit_2d_uint(
    BlitVertexData in [[stage_in]],
    texture2d<uint> tex2D [[ texture(0) ]],
    sampler sampler2D [[ sampler(0) ]]
) {
  return tex2D.sample(sampler2D, in.uv.xy, level(in.uv.w));
}

fragment int4 ps_blit_2d_int(
    BlitVertexData in [[stage_in]],
    texture2d<int> tex2D [[ texture(0) ]],
    sampler sampler2D [[ sampler(0) ]]
) {
  return tex2D.sample(sampler2D, in.uv.xy, level(in.uv.w));
}

fragment float4 ps_blit_2d_array_float(
    BlitVertexData in [[stage_in]],
    texture2d_array<float> tex2DArray [[ texture(0) ]],
    sampler sampler2D [[ sampler(0) ]]
) {
  return tex2DArray.sample(sampler2D, in.uv.xy, uint(in.uv.z), level(in.uv.w));
}
fragment float4 ps_blit_3d_float(
    BlitVertexData in [[stage_in]],
    texture3d<float> tex3D [[ texture(0) ]],
    sampler sampler2D [[ sampler(0) ]]
) {
  return tex3D.sample(sampler2D, in.uv.xyz, level(in.uv.w));
}

// -------------- Buffer Fill/Copy -------------- //

typedef struct {
    uint value;
    uint length;
} FillBufferValue;

kernel void cs_fill_buffer(
    device uint *buffer [[ buffer(0) ]],
    constant FillBufferValue &fill [[ buffer(1) ]],
    uint index [[ thread_position_in_grid ]]
) {
    if (index < fill.length) {
        buffer[index] = fill.value;
    }
}

kernel void cs_copy_buffer(
    device uchar *dest [[ buffer(0) ]],
    device uchar *source [[ buffer(1) ]],
    constant uint &size [[ buffer(2) ]],
    uint index [[ thread_position_in_grid ]]
) {
    if (index < size) {
        dest[index] = source[index];
    }
}
