#version 450

in vec2 a_Uv;
out vec4 Target0;

uniform sampler2D u_Texture;

void main() {
    Target0 = texture(u_Texture, a_Uv);
}
