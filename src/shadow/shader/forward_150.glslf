#version 150 core

const int MAX_LIGHTS = 10;

struct Light {
	vec4 pos;
	vec4 color;
	mat4 proj;
};

uniform b_Lights {
	Light u_Lights[MAX_LIGHTS];
};

uniform sampler2DArrayShadow t_Shadow;
uniform int u_NumLights;
uniform vec4 u_Color;

in vec3 v_Normal;
in vec3 v_Position;
out vec4 o_Color;

void main() {
	vec3 normal = normalize(v_Normal);
	vec3 ambient = vec3(0.05, 0.05, 0.05);
	vec3 color = ambient;
	for (int i=0; i<u_NumLights; ++i) {
		Light light = u_Lights[i];
		vec4 light_local = light.proj * vec4(v_Position, 1.0);
		light_local.xyw = (light_local.xyz/light_local.w + 1.0) / 2.0;
		light_local.z = i + 0.5;
		float shadow = texture(t_Shadow, light_local);
		vec3 light_dir = normalize(light.pos.xyz - v_Position);
		float diffuse = max(0.0, dot(normal, light_dir));
		color += shadow * diffuse * light.color.xyz;
	}
    o_Color = vec4(color, 1.0) * u_Color;
}
