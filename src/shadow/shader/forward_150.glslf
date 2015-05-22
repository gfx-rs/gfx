#version 150 core

const vec3 c_LightPos = vec3(5.0, 2.0, 3.0);
const vec3 c_LightColor = vec3(1.0, 1.0, 1.0);

in vec3 v_Normal;
out vec4 o_Color;
uniform vec4 u_Color;

void main() {
	vec3 normal = normalize(v_Normal);
	vec3 light = normalize(c_LightPos);
	vec3 ambient = vec3(0.1, 0.1, 0.1);
	float diffuse = max(0.0, dot(normal, light));
    o_Color = vec4((ambient + diffuse * c_LightColor) * u_Color.xyz, u_Color.a);
}
