#version 150 core

in ivec4 a_Pos;
in ivec4 a_Normal;

// world-space normal
out vec3 v_Normal;
// world-space position
out vec3 v_Position;

uniform VsLocals {
	// model-view-projection matrix
	mat4 u_Transform;
	// model matrix
	mat4 u_ModelTransform;
};

void main() {
	v_Normal = mat3(u_ModelTransform) * vec3(a_Normal.xyz);
	v_Position = (u_ModelTransform * vec4(a_Pos)).xyz;
    gl_Position = u_Transform * vec4(a_Pos);
}
