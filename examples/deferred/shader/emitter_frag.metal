#include <metal_stdlib>

using namespace metal;

struct FragmentOut {
    float4 main [[ color(0) ]];
};

fragment FragmentOut frag()
{
    FragmentOut out;

    out.main = float4(1.0);

    return out;
};

