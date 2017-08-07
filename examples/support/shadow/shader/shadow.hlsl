cbuffer Locals {
    float4x4 u_Transform;
};

float4 Vertex(int4 pos : a_Pos): SV_Position {
    float4 ndc = mul(u_Transform, float4(pos));
    ndc.z = (ndc.z + ndc.w) * 0.5;
    return ndc;
}

void Pixel() {}
