#include <metal_stdlib>

using namespace metal;

constant int MAX_LIGHTS = 10;

struct Light {
    float4 pos;
    float4 color;
    float4x4 proj;
};

struct LightArray {
    Light lights[MAX_LIGHTS];
};

struct Locals {
    float4 u_Color;
    int u_NumLights;
    int _padding[3];
};

struct FragmentIn {
    float4 pos [[ position ]];
    float3 position;
    float3 normal;
};

struct FragmentOut {
    float4 main [[ color(0) ]];
};

constexpr sampler shadow_sampler;

fragment FragmentOut frag(constant Locals& PsLocals     [[ buffer(2) ]],
                          constant LightArray& b_Lights [[ buffer(3) ]],
                          depth2d_array<float, access::sample> t_Shadow [[ texture(0) ]],
                          FragmentIn in                 [[ stage_in ]])
{
    FragmentOut out;

    float3 normal = normalize(in.normal);
    float3 ambient = float3(0.05, 0.05, 0.05);
    float3 color = ambient;

    for (int i = 0; i < min(PsLocals.u_NumLights, MAX_LIGHTS); ++i) {
        Light light = b_Lights.lights[i];

        float4 light_local = b_Lights.lights[i].proj * float4(in.position, 1.0);
        light_local.xyw = (light_local.xyz / light_local.w + 1.0) / 2.0;
        light_local.z = i;

        float shadow = t_Shadow.sample(shadow_sampler, light_local.xy, light_local.z);
        float3 light_dir = normalize(light.pos.xyz - in.position);
        float diffuse = max(0.0, dot(normal, light_dir));

        color += shadow; // * light.color.xyz;
    }

    out.main = PsLocals.u_Color;//float4(color, 1.0) * PsLocals.u_Color;//float4(1.0, 0.5, 0.2, 1.0);//float4(color, 1.0) * PsLocals.u_Color;

    return out;
};

