struct VsOutput {
	float4 pos: SV_Position;
	float3 uv: TEXCOORD;
};

cbuffer Locals {
	float4x4 u_InvProj;
	float4x4 u_WorldToCamera;
};

VsOutput Vertex(float2 in_pos: a_Pos) {
	float4x4 invModelView = transpose(u_WorldToCamera);
	float4 pos = float4(in_pos, 0.0, 1.0);
	float3 unProjected = mul(u_InvProj, pos).xyz;
	float3 uv = mul(invModelView, float4(unProjected, 0.0)).xyz;
	VsOutput output = { pos, uv };
	return output;
}

TextureCube<float4> t_Cubemap;
SamplerState t_Cubemap_;

float4 Pixel(VsOutput pin): SV_Target {
	return t_Cubemap.Sample(t_Cubemap_, pin.uv);
}
