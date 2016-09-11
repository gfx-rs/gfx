cbuffer VsLocals {
    float4x4 u_Transform;
    float4x4 u_ModelTransform;
}

cbuffer PsLocals {
    float4 u_Color;
    int u_NumLights;
};

struct Light {
	float4 pos;	// world position
	float4 color;
	float4x4 proj;	// view-projection matrix
};

static const int MAX_LIGHTS = 10;

cbuffer b_Lights {
	Light u_Lights[MAX_LIGHTS];
}

Texture2DArray<float> t_Shadow;
SamplerComparisonState t_Shadow_;

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
	float3 normal = normalize(In.world_normal);
	float3 ambient = float3(0.05, 0.05, 0.05);
	// accumulated color
	float3 color = ambient;
	for (int i=0; i<u_NumLights && i<MAX_LIGHTS; ++i) {
		Light light = u_Lights[i];
		// project into the light space
		float4 light_local = mul(light.proj, float4(In.world_pos, 1.0));
		// compute texture coordinates for shadow lookup
		light_local.xyw = (light_local.xyz/light_local.w + 1.0) / 2.0;
		light_local.y = 1.0 - light_local.y;
		light_local.z = i;
		// do the lookup, using HW PCF and comparison
		float shadow = t_Shadow.SampleCmpLevelZero(t_Shadow_, light_local.xyz, light_local.w);
		// compute Lambertian diffuse term
		float3 light_dir = normalize(light.pos.xyz - In.world_pos);
		float diffuse = max(0.0, dot(normal, light_dir));
		// add light contribution
		color += shadow * diffuse * light.color.xyz;
	}
	// multiply the light by material color
    return float4(color, 1.0) * u_Color;
}
