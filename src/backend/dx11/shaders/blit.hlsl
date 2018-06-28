cbuffer Region : register(b0) {
    float2 offset;
    float2 extent;
    float z;
    float level;
};

struct VsOutput {
    float4 pos: SV_POSITION;
    float4 uv: TEXCOORD0;
};

// Create a screen filling triangle
VsOutput vs_blit_2d(uint id: SV_VertexID) {
    float2 coord = float2((id << 1) & 2, id & 2);
    VsOutput output = {
        float4(float2(-1.0, 1.0) + coord * float2(2.0, -2.0), 0.0, 1.0),
        float4(offset + coord * extent, z, level)
    };
    return output;
}

SamplerState BlitSampler : register(s0);

Texture2D<uint4>  BlitSrc_Uint  : register(t0);
Texture2D<int4>   BlitSrc_Sint  : register(t0);
Texture2D<float4> BlitSrc_Float : register(t0);

// TODO: get rid of GetDimensions call
uint4 Nearest_Uint(float4 uv)
{
    float2 size;
    BlitSrc_Uint.GetDimensions(size.x, size.y);

    float2 pix = uv.xy * size;

    return BlitSrc_Uint[int2(pix)].xyzw;
}

int4 Nearest_Sint(float4 uv)
{
    float2 size;
    BlitSrc_Sint.GetDimensions(size.x, size.y);

    float2 pix = uv.xy * size;

    return BlitSrc_Sint[int2(pix)].xyzw;
}

uint4 ps_blit_2d_uint(VsOutput input) : SV_Target
{
    return Nearest_Uint(input.uv);
}

int4 ps_blit_2d_int(VsOutput input) : SV_Target
{
    return Nearest_Sint(input.uv);
}

float4 ps_blit_2d_float(VsOutput input) : SV_Target
{
    return BlitSrc_Float.Sample(BlitSampler, input.uv);
}
