#define SCREEN 0
#define DODGE 1
#define BURN 2
#define OVERLAY 3
#define MULTIPLY 4
#define ADD 5
#define DIVIDE 6
#define GRAIN_EXTRACT 7
#define GRAIN_MERGE 8

struct VsOutput {
	float4 pos: SV_Position;
	float2 uv: TEXCOORD;
};
 
VsOutput Vertex(float2 pos: a_Pos, float2 uv: a_Uv) {
	VsOutput output = {
		float4(pos, 0.0, 1.0),
		uv,
	};
	return output;
}

cbuffer Locals {
	int u_Blend;
};

Texture2D<float> t_Lena;
SamplerState t_Lena_;
Texture2D<float3> t_Tint;
SamplerState t_Tint_;

float4 Pixel(VsOutput pin): SV_Target {
	// we sample from both textures using the same uv coordinates. since our
	// lena image is grayscale, we only get the first component.
	float3 lena = t_Lena.Sample(t_Lena_, pin.uv);
	float3 tint = t_Tint.Sample(t_Tint_, pin.uv);

	float3 result = 0.0.xxx;

	// normally you'd have a shader program per technique, but for the sake of
	// simplicity we'll just branch on it here.
	switch (u_Blend) {
		case SCREEN:
			result = 1.0.xxx - (1.0.xxx - lena) * (1.0.xxx - tint);
			break;
		case DODGE:
			result = lena / (1.0.xxx - tint);
			break;
		case BURN:
			result = 1.0.xxx - ((1.0.xxx - lena) / lena);
			break;
		case OVERLAY:
			result = lena * (lena + (tint * 2) * (1.0.xxx - lena));
			break;
		case MULTIPLY:
			result = lena * tint;
			break;
		case ADD:
			result = lena + tint;
			break;
		case DIVIDE:
			result = lena / tint;
			break;
		case GRAIN_EXTRACT:
			result = lena - tint + 0.5;
			break;
		case GRAIN_MERGE:
			result = lena + tint - 0.5;
			break;
	}

	return float4(result, 1.0);
}
