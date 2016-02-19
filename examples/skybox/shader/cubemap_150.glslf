#version 150 core

uniform samplerCube t_Cubemap;

in vec3 v_Uv;

out vec4 o_Color;

void main() {
    o_Color = vec4(texture(t_Cubemap, v_Uv));
}
