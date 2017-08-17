cbuffer Locals {
    float4x4 u_Model;
    float4x4 u_View;
    float4x4 u_Proj;
};

struct VsOutput {
    float4 pos: SV_Position;
    float3 color: COLOR;
};

VsOutput Vertex(float3 pos : a_Pos, float3 color : a_Color) {

    VsOutput output ;
    output.pos = float4(pos, 1.0);
    output.color = color;
    return output;
}

float4 Pixel(VsOutput pin) : SV_Target {
    return float4(pin.color, 1.0);
}



// This allows us to compile the shader with a #define to choose
// the different partition modes for the hull shader.
// See the hull shader: [partitioning(BEZIER_HS_PARTITION)]
// This sample demonstrates "integer", "fractional_even", and "fractional_odd"
#ifndef HS_PARTITION
#define HS_PARTITION "integer"
#endif //HS_PARTITION


//----------------------------------------------------------------------------------
// Constant data function for the HS.  This is executed once per patch.
//--------------------------------------------------------------------------------------
struct HS_CONSTANT_DATA_OUTPUT
{
    float Edges[4]            : SV_TessFactor;
    float Inside [2]          : SV_InsideTessFactor;
};


HS_CONSTANT_DATA_OUTPUT ConstantHS( InputPatch<VsOutput, 4> ip,
                                    uint PatchID : SV_PrimitiveID )
{
	float g_fTessellationFactor = 8.0;
    HS_CONSTANT_DATA_OUTPUT Output;

    Output.Edges[0] = Output.Edges[1] = Output.Edges[2] = Output.Edges[3] = g_fTessellationFactor;
    Output.Inside [0] = Output.Inside [1] = g_fTessellationFactor;

    return Output;
}

// The hull shader is called once per output control point, which is specified with
// outputcontrolpoints.  For this sample, we take the control points from the vertex
// shader and pass them directly off to the domain shader.  In a more complex scene,
// you might perform a basis conversion from the input control points into a Bezier
// patch, such as the SubD11 Sample of DirectX SDK.

// The input to the hull shader comes from the vertex shader

// The output from the hull shader will go to the domain shader.
// The tessellation factor, topology, and partition mode will go to the fixed function
// tessellator stage to calculate the UV and domain points.

[domain("quad")] //Quad domain for our shader
[partitioning(HS_PARTITION)] //Partitioning type according to the GUI
[outputtopology("triangle_cw")] //Where the generated triangles should face
[outputcontrolpoints(4)] //Number of times this part of the hull shader will be called for each patch
[patchconstantfunc("ConstantHS")] //The constant hull shader function
VsOutput HS( InputPatch<VsOutput, 4> p,
                    uint i : SV_OutputControlPointID,
                    uint PatchID : SV_PrimitiveID )
{
    VsOutput Output;
    Output.pos = p[i].pos;
    Output.color = p[i].color;
    return Output;
}


//Domain Shader is invoked for each vertex created by the Tessellator
[domain("quad")]
VsOutput DS( HS_CONSTANT_DATA_OUTPUT input,
                    float2 UV : SV_DomainLocation,
                    const OutputPatch<VsOutput, 4> quad )
{
    VsOutput Output;

	//Interpolation to find each position the generated vertices
	float3 verticalPos1 = lerp(quad[0].pos,quad[1].pos,UV.y);
	float3 verticalPos2 = lerp(quad[3].pos,quad[2].pos,UV.y);
	float3 finalPos = lerp(verticalPos1,verticalPos2,UV.x);

  float3 color1 = lerp(quad[0].color,quad[1].color,UV.y);
	float3 color2= lerp(quad[3].color,quad[2].color,UV.y);
	float3 finalColor = lerp(color1,color2,UV.x);

	// IT WORKS float4 p = mul(u_Proj, mul(u_View, float4(inner, 1.0)));

//    Output.vPosition = mul( float4(finalPos,1), (u_View) );
Output.pos = mul(u_Proj, mul(u_View, mul(u_Model, float4(finalPos, 1.0))));
Output.color = finalColor;

    return Output;
}
