struct VsInput {
	float2 pos: a_Position;
	float2 trans: a_Translate;
	uint color: a_Color;
};

struct VsOutput {
	float4 pos: SV_Position;
    float4 color: COLOR;
};

cbuffer Locals {
	float u_Scale;
};
 
VsOutput Vertex(VsInput In) {
	uint4 color = In.color >> uint4(24, 16, 8, 0) & 0x000000FFu;
    VsOutput output = {
    	float4((In.pos * u_Scale) + In.trans, 0.0, 1.0),
        float4(color) / 255.0,
    };
    return output;
}

float4 Pixel(VsOutput pin) : SV_Target {
    return pin.color;
}
