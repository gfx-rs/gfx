#version 150 core

layout(std140)
uniform LightLocals {
	vec4 u_CamPosAndRadius;
};
uniform sampler2D t_Position;
uniform sampler2D t_Normal;
uniform sampler2D t_Diffuse;
in vec3 v_LightPos;
out vec4 Target0;

void main() {
	ivec2 itc = ivec2(gl_FragCoord.xy);
	vec3 pos     = texelFetch(t_Position, itc, 0).xyz;
	vec3 normal  = texelFetch(t_Normal,   itc, 0).xyz;
	vec3 diffuse = texelFetch(t_Diffuse,  itc, 0).xyz;

	vec3 light    = v_LightPos;
	vec3 to_light = normalize(light - pos);
	vec3 to_cam   = normalize(u_CamPosAndRadius.xyz - pos);

	vec3 n = normalize(normal);
	float s = pow(max(0.0, dot(to_cam, reflect(-to_light, n))), 20.0);
	float d = max(0.0, dot(n, to_light));

	float dist_sq = dot(light - pos, light - pos);
	float scale = max(0.0, 1.0 - dist_sq * u_CamPosAndRadius.w);

	vec3 res_color = d * diffuse + vec3(s);

	Target0 = vec4(scale*res_color, 1.0);
}
