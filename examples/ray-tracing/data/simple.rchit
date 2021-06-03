#version 460
#extension GL_EXT_ray_tracing : enable

layout(location = 0) rayPayloadInEXT vec3 out_color;
hitAttributeEXT vec3 attribs;

void main() {
  out_color = vec3(1.0, 0.0, 0.0);
}
