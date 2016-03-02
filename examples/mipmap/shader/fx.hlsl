struct VsOutput {
    float4 pos: SV_Position;
    float2 uv: TEXCOORD0;
};

Texture2D<float4> Texture: t_Tex;
SamplerState Sampler: t_Tex;
 
VsOutput Vertex(float2 pos : a_Pos, float2 uv : a_Uv) {
    VsOutput output = {
        float4(pos, 0.0, 1.0),
        uv,
    };
    return output;
}

float4 Pixel(VsOutput pin) : SV_Target {
    return Texture.Sample(Sampler, pin.uv);
}
