#version 150 core

const int MAX_LIGHTS = 10;

struct Light {
	vec4 pos;	// world position
	vec4 color;
	mat4 proj;	// view-projection matrix
};

uniform b_Lights {
	Light u_Lights[MAX_LIGHTS];
};

// an array of shadows, one per light
uniform sampler2DArrayShadow t_Shadow;
// active number of lights
uniform int u_NumLights;
// material color
uniform vec4 u_Color;

in vec3 v_Normal;
in vec3 v_Position;
out vec4 o_Color;

void main() {
	vec3 normal = normalize(v_Normal);
	vec3 ambient = vec3(0.05, 0.05, 0.05);
	// accumulated color
	vec3 color = ambient;
	for (int i=0; i<u_NumLights; ++i) {
		Light light = u_Lights[i];
		// project into the light space
		vec4 light_local = light.proj * vec4(v_Position, 1.0);
		// compute texture coordinates for shadow lookup
		light_local.xyw = (light_local.xyz/light_local.w + 1.0) / 2.0;
		light_local.z = i + 0.5;
		// do the lookup, using HW PCF and comparison
		float shadow = texture(t_Shadow, light_local);
		// compute Lambertian diffuse term
		vec3 light_dir = normalize(light.pos.xyz - v_Position);
		float diffuse = max(0.0, dot(normal, light_dir));
		// add light contribution
		color += shadow * diffuse * light.color.xyz;
	}
	// multiply the light by material color
    o_Color = vec4(color, 1.0) * u_Color;
}
