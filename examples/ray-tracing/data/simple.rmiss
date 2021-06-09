#version 460
#extension GL_EXT_ray_tracing : enable

layout(location = 0) rayPayloadInEXT vec3 out_color;

void main() { out_color = vec3(0.8, 0.8, 0.8); }