#include <metal_stdlib>

using namespace metal;

struct VertexOut {
    float4 pos [[ position ]];
    float3 light_pos;
};

struct FragmentOut {
    float4 main [[ color(0) ]];
};

struct Light {
    packed_float3 pos;
    float radius;
};

fragment FragmentOut frag(constant Light&                LightLocals [[ buffer(0) ]],
                          texture2d<float, access::read> t_Position  [[ texture(0) ]],
                          //sampler   t_Position [[ sampler(0) ]],
                          texture2d<float, access::read> t_Normal    [[ texture(1) ]],
                          //sampler   t_Normal   [[ sampler(1) ]],
                          texture2d<float, access::read> t_Diffuse   [[ texture(2) ]],
                          //sampler   t_Diffuse  [[ sampler(2) ]],
                          VertexOut                      in          [[ stage_in ]])
{
    FragmentOut out;

    uint2 itc = uint2(in.pos.xy);

    float3 pos =     t_Position.read(itc).xyz;
    float3 normal =  t_Normal.read(itc).xyz;
    float3 diffuse = t_Diffuse.read(itc).xyz;

    float3 light = in.light_pos;
    float3 to_light = normalize(light - pos);
    float3 to_cam = normalize(LightLocals.pos - pos);

    float3 n = normalize(normal);
    float s = pow(max(0.0, dot(to_cam, reflect(-to_light, n))), 20.0);
    float d = max(0.0, dot(n, to_light));

    float dist_sq = dot(light - pos, light - pos);
    float scale = max(0.0, 1.0 - dist_sq * LightLocals.radius);

    float3 res_color = d * diffuse + float3(s);

    out.main = float4(scale * res_color, 1.0);

    return out;
};
