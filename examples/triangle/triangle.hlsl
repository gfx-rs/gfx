cbuffer locals {
	float4x4 mvp;
};

struct VsOutput {
	float4 pos: SV_Position;
	float3 color: COLOR;
};
 
VsOutput Vertex(float2 a_Pos : POSITION, float3 a_Color : COLOR) {
 	float4 pos = float4(a_Pos, 0.0, 1.0);
 	VsOutput output = {
    	mul(pos, mvp),
    	a_Color,
    };
 	return output;
}

float4 Pixel(float3 color: COLOR) : SV_Target {
	return float4(color, 1.0);
}
