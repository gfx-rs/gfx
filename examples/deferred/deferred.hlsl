// Terrain program

struct TerrainInput {
	float3 pos: a_Pos;
	float3 normal: a_Normal;
	float3 color: a_Color;
};

struct TerrainVarying {
	float4 pos: SV_Position;
	float3 frag_pos: POSITION;
	float3 normal: NORMAL;
	float3 color: COLOR;
};

struct TerrainOutput {
	float4 pos: SV_Target0;
	float4 normal: SV_Target1;
	float4 color: SV_Target2;
};

cbuffer TerrainLocals {
	float4x4 Model: u_Model;
	float4x4 View: u_View;
	float4x4 Proj: u_Proj;
};
 
TerrainVarying TerrainVs(TerrainInput In) {
	float4 fpos = mul(Model, float4(In.pos, 1.0));
	TerrainVarying output = {
		mul(Proj, mul(View, fpos)),
		fpos.xyz,
		mul(Model, float4(In.normal, 0.0)).xyz,
		In.color,
	};
	return output;
}

TerrainOutput TerrainPs(TerrainVarying In) {
	TerrainOutput output = {
		float4(In.frag_pos, 0.0),
		float4(normalize(In.normal), 0.0),
		float4(In.color, 1.0),
	};
	return output;
}

// Blit program

Texture2D<float4> BlitTexture: u_BlitTex;
SamplerState BlitSampler: u_BlitTex;

struct BlitVarying {
	float4 pos: SV_Position;
	float2 tc: TEXCOORD;
};

BlitVarying BlitVs(int3 pos: a_Pos, int2 tc: a_TexCoord) {
	BlitVarying output = {
		float4(pos, 1.0),
		tc,
	};
	return output;
}

float4 BlitPs(BlitVarying In): SV_Target {
	return BlitTexture.Sample(BlitSampler, In.tc);
}

// common parts

cbuffer CubeLocals {
	float4x4 Transform: u_Transform;
	float Radius: u_Radius;
};

#define NUM_LIGHTS	250
cbuffer u_LightPosBlock {
	float4 offs[NUM_LIGHTS];
};

// Light program

cbuffer LightLocals {
	float RadiusM2: u_RadiusM2;
	float3 CamPos: u_CameraPos;
	float2 FrameRes: u_FrameRes;
};

struct LightVarying {
	float4 pos: SV_Position;
	float3 light_pos: POSITION;
};

LightVarying LightVs(int3 pos: a_Pos, uint inst_id: SV_InstanceID) {
	float3 lpos = offs[inst_id].xyz;
	LightVarying output = {
		mul(Transform, float4(Radius * float3(pos) + lpos, 1.0)),
		lpos,
	};
	return output;
}

float4 LightPs(LightVarying In): SV_Target {
	return float4(1.0,0.0,0.0,1.0); //TODO
}

// Emitter program

float4 EmitterVs(int3 pos: a_Pos, uint inst_id: SV_InstanceID): SV_Position {
	float3 lpos = offs[inst_id].xyz;
	return mul(Transform, float4(Radius * float3(pos) + lpos, 1.0));
}

float4 EmitterPs(): SV_Target {
	return float4(1.0,1.0,1.0,1.0);
}
