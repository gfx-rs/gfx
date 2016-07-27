#version 150 core

in ivec3 a_Pos;

layout(std140)
uniform CubeLocals {
	mat4 u_Transform;
	float u_Radius;
};

struct LightInfo {
	vec4 pos;
};

const int NUM_LIGHTS = 250;
layout(std140)
uniform LightPosBlock {
	LightInfo u_Lights[NUM_LIGHTS];
};

void main() {
	gl_Position = u_Transform * vec4(u_Radius * a_Pos + u_Lights[gl_InstanceID].pos.xyz, 1.0);
}
