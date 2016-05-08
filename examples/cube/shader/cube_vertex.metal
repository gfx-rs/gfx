#include <metal_stdlib>

using namespace metal;

typedef struct {
    char4 pos;
    char2 coords;
} Vertex;

typedef struct {
    mat4 transform;
} Locals;

typedef struct {
    float4 pos [[ position ]];
    float2 coords;
} VertexOut;

vertex VertexOut vert(constant Locals locals      [[ buffer(0) ]],
                      device Vertex* vertex_array [[ buffer(1) ]],
                      unsigned int vid            [[ vertex_id ]])
{
    VertexOut out;

    Vertex v = vertex_array[vid];

    out.pos = locals.transform * v.pos;
    out.coords = v.coords;

    return out;
}

