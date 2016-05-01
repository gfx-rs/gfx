#include <metal_stdlib>

using namespace metal;

struct ColorInOut {
    float4 position [[position]];
    float4 color;
};

// fragment shader function
fragment float4 frag(ColorInOut in [[stage_in]])
{
    return in.color;
};

