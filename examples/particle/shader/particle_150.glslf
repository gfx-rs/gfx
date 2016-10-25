#version 150 core

in VertexData {
    vec4 color;
    vec2 uv;
} VertexIn;

out vec4 Target0;

void main() {
    float alpha = max(1-dot(VertexIn.uv, VertexIn.uv), 0);
    Target0 = vec4(VertexIn.color.xyz, VertexIn.color.w*alpha);
}
