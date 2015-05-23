#version 150 core

in vec3 a_Pos;
in vec3 a_Normal;

// world-space normal
out vec3 v_Normal;
// world-space position
out vec3 v_Position;

// model-view-projection matrix
uniform mat4 u_Transform;
// model matrix
uniform mat4 u_ModelTransform;

void main() {
	v_Normal = mat3(u_ModelTransform) * a_Normal;
	v_Position = (u_ModelTransform * vec4(a_Pos, 1.0)).xyz;
    gl_Position = u_Transform * vec4(a_Pos, 1.0);
}
