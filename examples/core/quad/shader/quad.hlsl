Texture2D u_Texture: register(t0, space0);
SamplerState u_Sampler: register(s1, space1);

struct VsOutput {
    float4 pos: SV_POSITION;
    float2 uv: TEXCOORD;
};

VsOutput vs_main(float2 pos: ATTRIB0, float2 uv: ATTRIB1) {
    // Texture coordinates are in OpenGL/Vulkan (origin bottom left)
    // convert them to HLSL (origin top left)
    VsOutput output = {
        float4(pos, 0.0, 1.0),
        float2(uv.x, 1.0 - uv.y)
    };
    return output;
}

float4 ps_main(VsOutput input) : SV_TARGET {
    return u_Texture.Sample(u_Sampler, input.uv);
}
