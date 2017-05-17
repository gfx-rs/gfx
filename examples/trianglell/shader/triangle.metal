using namespace metal;

struct VsInput {
  float3 a_Pos [[attribute(0)]];
  float3 a_Color [[attribute(1)]];
};

struct VsOutput {
  float4 pos [[position]];
  float3 color;
};

vertex VsOutput vs_main(VsInput in [[stage_in]]) {
  VsOutput out;
  out.pos = float4(in.a_Pos, 1.0);
  out.color = in.a_Color;
  return out;
}

fragment float4 ps_main(VsOutput in [[stage_in]]) {
  return float4(in.color, 1.0);
}

