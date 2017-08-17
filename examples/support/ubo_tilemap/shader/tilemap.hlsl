struct VsOutput {
	float4 pos: SV_Position;
	float2 buf_pos: TEXCOORD;
};

cbuffer b_VsLocals {
	float4x4 u_Model;
	float4x4 u_View;
	float4x4 u_Proj;
};
 
VsOutput Vertex(float4 pos: a_Pos, float2 buf_pos: a_BufPos) {
	VsOutput output = {
		mul(u_Proj, mul(u_View, mul(u_Model, pos))),
		buf_pos,
	};
	return output;
}

struct TileMapData {
    float4 data;
};
#define TILEMAP_BUF_LENGTH 2304
cbuffer b_TileMap {
    TileMapData u_Data[TILEMAP_BUF_LENGTH];
};

cbuffer b_PsLocals {
    float4 u_WorldSize;
    float4 u_TilesheetSize;
    float2 u_TileOffsets;
};

Texture2D<float4> t_TileSheet;
SamplerState t_TileSheet_;

float4 Pixel(VsOutput pin): SV_Target0 {
	// apply offset to v_BufPos
    float2 offset_bufpos = pin.buf_pos + (u_TileOffsets / u_WorldSize.zz);
    // base coordinates for the charmap tile of the "nearest" (left/down) vertex.
    float2 bufTileCoords = floor(offset_bufpos);

    // "raw" offset, expressed as 0.0..1.0, for the offset position of the current
    // fragment
    // -- need to flip the y coords
    float2 rawUvOffsets = float2(offset_bufpos.x - bufTileCoords.x, 1.0 - (offset_bufpos.y - bufTileCoords.y));

    if (bufTileCoords.x >= 0.0 && bufTileCoords.x < u_WorldSize.x && bufTileCoords.y >= 0.0 && bufTileCoords.y < u_WorldSize.y) {
        int bufIdx = int((bufTileCoords.y * u_WorldSize.x) + bufTileCoords.x);
        float4 entry = u_Data[bufIdx].data;

        float2 uvCoords = (entry.xy + rawUvOffsets) / u_TilesheetSize.xy;
        return t_TileSheet.Sample(t_TileSheet_, uvCoords);
    } else {
        // if we're here it means the buftilecoords are outside the buffer, so let's just show black
        return float4(0.0,0.0,0.0,1.0);
    }
}
