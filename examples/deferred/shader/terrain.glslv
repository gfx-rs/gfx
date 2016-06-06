#version 150 core

layout(std140)
uniform TerrainLocals {
	mat4 u_Model;
	mat4 u_View;
	mat4 u_Proj;
};
in vec3 a_Pos;
in vec3 a_Normal;
in vec3 a_Color;
out vec3 v_FragPos;
out vec3 v_Normal;
out vec3 v_Color;

void main() {
	v_FragPos = (u_Model * vec4(a_Pos, 1.0)).xyz;
	v_Normal = mat3(u_Model) * a_Normal;
	v_Color = a_Color;
	gl_Position = u_Proj * u_View * u_Model * vec4(a_Pos, 1.0);
}
