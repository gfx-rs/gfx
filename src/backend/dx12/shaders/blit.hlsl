
Texture2DArray blit_src : register(t0);
SamplerState blit_sampler : register(s0);

cbuffer region : register(b0) {
    float2 src_start;
    float2 src_end;
    float z;
};

struct VsOutput {
    float4 position : SV_POSITION;
    float3 src_coords : TEXCOORD;
};

// Create a screen filling triangle
VsOutput vs_blit_2d(uint id: SV_VertexID) {
    VsOutput output;
    switch (id) {
        case 0:
            output.position = float4(-1.0, -1.0, 0.0, 0.0);
            output.src_coords = float3(src_start, z);
            break;
        case 1:
            output.position = float4(0.0, 2.0, 0.0, 0.0);
            output.src_coords = float3(src_start.x, 2 * src_end.y - src_start.y, z);
            break;
        default:
            output.position = float4(2.0, 0.0, 0.0, 0.0);
            output.src_coords = float3(2 * src_end.x - src_start.x, src_start.y, z);
            break;
    }
    return output;
}

float4 ps_blit_2d(VsOutput input) : SV_TARGET0 {
    return blit_src.Sample(blit_sampler, input.src_coords);
}
