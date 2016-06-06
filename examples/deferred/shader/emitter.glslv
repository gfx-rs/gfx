#version 150 core

in ivec3 a_Pos;

layout(std140)
uniform CubeLocals {
	mat4 u_Transform;
	float u_Radius;
};

const int NUM_LIGHTS = 250;
layout(std140)
uniform u_LightPosBlock {
	vec4 offs[NUM_LIGHTS];
};

void main() {
	gl_Position = u_Transform * vec4(u_Radius * a_Pos + offs[gl_InstanceID].xyz, 1.0);
}
