cbuffer locals {
	float4x4 mvp;
};

struct VsOutput {
	float4 pos: SV_Position;
	float3 color: COLOR;
};
 
VsOutput Vertex(float2 pos : a_Pos, float3 color : a_Color) {
 	VsOutput output = {
    	mul(float4(pos, 0.0, 1.0), mvp),
    	color,
    };
 	return output;
}

float4 Pixel(float3 color: COLOR) : SV_Target {
	return float4(color, 1.0);
}
