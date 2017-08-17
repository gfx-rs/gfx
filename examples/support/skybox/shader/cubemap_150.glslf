#version 150 core

uniform samplerCube t_Cubemap;

in vec3 v_Uv;

out vec4 Target0;

void main() {
    Target0 = vec4(texture(t_Cubemap, v_Uv));
}
