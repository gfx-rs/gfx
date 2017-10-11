using namespace metal;

struct VsInput {
  float2 a_Pos [[attribute(0)]];
  float2 a_Uv [[attribute(1)]];
};

struct VsOutput {
  float4 pos [[position]];
  float2 uv;
};

vertex VsOutput vs_main(VsInput in [[stage_in]]) {
  VsOutput out;
  out.pos = float4(in.a_Pos, 0.0, 1.0);
  // Texture coordinates are in OpenGL/Vulkan (origin bottom left)
  // convert them to Metal (origin top left)
  out.uv = float2(in.a_Uv.x, 1 - in.a_Uv.y);
  return out;
}

struct PixelArg0 {
  texture2d<float> tex2D [[ id(0) ]];
};
struct PixelArg1 {
  sampler sampler2D [[ id(0) ]];
};
                        
fragment float4 ps_main(VsOutput in [[stage_in]],
                        device PixelArg0* pixelArg0 [[ buffer(0) ]],
                        device PixelArg1* pixelArg1 [[ buffer(1) ]])
{
  return pixelArg0->tex2D.sample(pixelArg1->sampler2D, in.uv);
}
