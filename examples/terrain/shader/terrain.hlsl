cbuffer Locals {
    float4x4 u_Model;
    float4x4 u_View;
    float4x4 u_Proj;
};

struct VsOutput {
    float4 pos: SV_Position;
    float3 color: COLOR;
};

VsOutput Vertex(float3 pos : a_Pos, float3 color : a_Color) {
    float4 p = mul(u_Proj, mul(u_View, mul(u_Model, float4(pos, 1.0))));
    VsOutput output = { p, color };
    return output;
}

float4 Pixel(VsOutput pin) : SV_Target {
    return float4(pin.color, 1.0);
}
