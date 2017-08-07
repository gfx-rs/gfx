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
	float2 u_Offsets;
};

Texture2D<float4> t_Color;
Texture2D<float2> t_Flow;
Texture2D<float> t_Noise;
SamplerState t_Color_;
SamplerState t_Flow_;
SamplerState t_Noise_;

float4 Pixel(VsOutput pin): SV_Target {
	// we sample the direction from our flow map, then map it to a [-1, 1] range
	float2 flow = t_Flow.Sample(t_Flow_, pin.uv).rg * 2.0 - 1.0;

	// we apply some noise to get rid of the visible repeat pattern
	float noise = t_Noise.Sample(t_Noise_, pin.uv);
	
	// apply the noise to our cycles
	float2 phases = noise * 0.05 + u_Offsets * 0.25;

	// grab two samples to interpolate between
	float4 t0 = t_Color.Sample(t_Color_, pin.uv + flow * phases.x);
	float4 t1 = t_Color.Sample(t_Color_, pin.uv + flow * phases.y);

	float mix = 2.0 * abs(u_Offsets.x - 0.5);
	return lerp(t0, t1, mix);
}
