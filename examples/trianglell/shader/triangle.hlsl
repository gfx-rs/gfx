struct VsOutput {
    float4 pos: SV_POSITION;
    float3 color: COLOR;
};

VsOutput vs_main(float3 pos : a_Pos, float3 color : a_Color) {
    VsOutput output = {
        float4(pos, 1.0),
        color,
    };
    return output;
}

float4 ps_main(VsOutput input) : SV_TARGET {
    return float4(input.color, 1.0);
}
