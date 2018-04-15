#include <metal_stdlib>
using namespace metal;

typedef struct {
	float2 a_srcCoord [[attribute(0)]];
	float2 a_dstCoord [[attribute(1)]];
} TextureBlitAttributes;

typedef struct {
	float4 v_position [[position]];
} VertexData;

vertex VertexData testShader() {
	float4 pos = {.0f, .0f, .0f, .0f};
	return VertexData{pos};
}
