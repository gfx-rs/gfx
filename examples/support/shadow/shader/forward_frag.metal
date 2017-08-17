#include <metal_stdlib>

using namespace metal;

constant int MAX_LIGHTS = 10;

struct Light {
    float4 pos;
    float4 color;
    float4x4 proj;
};

struct Locals {
    float4 u_Color;
    int u_NumLights;
    int _padding[3];
};

struct FragmentIn {
    float3 position;
    float3 normal;
};

struct FragmentOut {
    float4 main [[ color(0) ]];
};

constexpr sampler s(coord::normalized, address::clamp_to_edge, filter::linear, compare_func::less_equal);

fragment FragmentOut frag(constant Locals& PsLocals [[ buffer(2) ]],
                          constant Light* b_Lights  [[ buffer(3) ]],

                          depth2d_array<float, access::sample> t_Shadow [[ texture(0) ]],
                          sampler t_Shadow_                             [[ sampler(0) ]],

                          FragmentIn in [[ stage_in ]])
{
    FragmentOut out;

    float3 normal = in.normal;
    float3 ambient = float3(0.05, 0.05, 0.05);
    float3 color = ambient;

    for (int i = 0; i < PsLocals.u_NumLights && i < MAX_LIGHTS; ++i) {
        Light light = b_Lights[i];

        float4 light_local = light.proj * float4(in.position, 1.0);
        light_local.xyw = (light_local.xyz / light_local.w + 1.0) / 2.0;
        light_local.y = 1.0 - light_local.y;

        float shadow = t_Shadow.sample_compare(s, light_local.xy, i, light_local.w);
        float3 light_dir = normalize(light.pos.xyz - in.position);
        float diffuse = max(0.0, dot(normal, light_dir));

        color += shadow * diffuse * b_Lights[i].color.xyz;
    }

    out.main = float4(color, 1.0) * PsLocals.u_Color;

    return out;
};

