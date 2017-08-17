struct VsOutput {
    float4 pos: SV_Position;
    float4 color: COLOR;
};

struct GsOutput {
    float4 pos: SV_Position;
    float4 color: COLOR;
    float2 uv: TEXCOORD;
};

cbuffer Locals {
	float u_Aspect;
};

#define PARTICLE_RADIUS 0.05

VsOutput VS(float2 pos: a_Pos, float4 color: a_Color) {
    VsOutput output = {
    	float4(pos, 0, 1),
    	color,
    };
    return output;
}

[maxvertexcount(4)]
void GS(point VsOutput p[1], inout TriangleStream<GsOutput> vs) {
	GsOutput v;
	v.color = p[0].color;
	v.pos = p[0].pos + float4(-PARTICLE_RADIUS*u_Aspect, -PARTICLE_RADIUS, 0, 0);
	v.uv = float2(-1, -1);
	vs.Append(v);
	v.pos = p[0].pos + float4(PARTICLE_RADIUS*u_Aspect, -PARTICLE_RADIUS, 0, 0);
	v.uv = float2(1, -1);
	vs.Append(v);
	v.pos = p[0].pos + float4(-PARTICLE_RADIUS*u_Aspect, PARTICLE_RADIUS, 0, 0);
	v.uv = float2(-1, 1);
	vs.Append(v);
	v.pos = p[0].pos + float4(PARTICLE_RADIUS*u_Aspect, PARTICLE_RADIUS, 0, 0);
	v.uv = float2(1, 1);
	vs.Append(v);
}

float4 PS(GsOutput v) : SV_Target {
	float alpha = max(1-dot(v.uv, v.uv), 0);
	return float4(v.color.xyz, v.color.w*alpha);
}
