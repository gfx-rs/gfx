cbuffer VsLocals {
    float4x4 u_Transform;
    float4x4 u_ModelTransform;
}

cbuffer PsLocals {
    int u_NumLights;
    float4 u_Color;
};

struct Light {
	float4 pos;	// world position
	float4 color;
	float4x4 proj;	// view-projection matrix
};

static const int MAX_LIGHTS = 10;

cbuffer Lights {
	Light u_Lights[MAX_LIGHTS];
}

struct VsOutput {
    float4 pos: SV_Position;
    float3 world_pos: POSITION;
    float3 world_normal: NORMAL;
};

VsOutput Vertex(int4 pos : a_Pos, int4 normal : a_Normal) {
	VsOutput Out = {
		mul(u_Transform, float4(pos)),
		mul(u_ModelTransform, float4(pos)).xyz,
		mul(u_ModelTransform, float4(normal)).xyz,
	};
    return Out;
}

float4 Pixel(VsOutput In): SV_Target {
	return float4(1.0,1.0,1.0,1.0);
}
