struct VsOutput {
    float4 pos: SV_Position;
    float3 color: COLOR;
};
 
VsOutput Vertex(float3 pos : a_Pos, float3 color : a_Color) {
    VsOutput output = {
        float4(pos, 1.0),
        color,
    };
    return output;
}

float4 Pixel(VsOutput pin) : SV_Target {
    return float4(pin.color, 1.0);
}
