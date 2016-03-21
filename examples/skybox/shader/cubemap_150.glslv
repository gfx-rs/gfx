#version 150 core

uniform Locals {
	mat4 u_InvProj;
	mat4 u_WorldToCamera;
};

in vec2 a_Pos;

out vec3 v_Uv;

void main() {
    mat3 invModelView = transpose(mat3(u_WorldToCamera));
    vec3 unProjected = (u_InvProj * vec4(a_Pos, 0.0, 1.0)).xyz;
    v_Uv = invModelView * unProjected;

    gl_Position = vec4(a_Pos, 0.0, 1.0);
}
