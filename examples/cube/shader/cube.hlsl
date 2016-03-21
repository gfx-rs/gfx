struct VsOutput {
    float4 pos: SV_Position;
    float2 tc: TEXCOORD;
};

cbuffer Locals {
	float4x4 u_Transform;
};

VsOutput Vertex(int4 pos: a_Pos, int2 tc: a_TexCoord) {
    VsOutput output = {
    	mul(u_Transform, pos),
    	tc,
    };
    return output;
}

Texture2D<float4> t_Color;
SamplerState t_Color_;

float4 Pixel(VsOutput pin) : SV_Target {
	float4 tex = t_Color.Sample(t_Color_, pin.tc);
    float blend = dot(pin.tc-0.5, pin.tc-0.5);
    return lerp(tex, 0.0, blend*1.0);   
}
