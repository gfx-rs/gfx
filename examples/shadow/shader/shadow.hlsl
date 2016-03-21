cbuffer Locals {
    float4x4 u_Transform;
};

float4 Vertex(int4 pos : a_Pos): SV_Position {
    return mul(u_Transform, float4(pos));
}

void Pixel() {}
