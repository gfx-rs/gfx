#version 150 core

in vec2 v_BufPos;

out vec4 Target0;

struct TileMapData {
    vec4 data;
};
const int TILEMAP_BUF_LENGTH = 2304;
uniform b_TileMap {
    TileMapData u_Data[TILEMAP_BUF_LENGTH];
};
uniform b_PsLocals {
    vec4 u_WorldSize;
    vec4 u_TilesheetSize;
    vec2 u_TileOffsets;
};
uniform sampler2D t_TileSheet;

void main() {
    // apply offset to v_BufPos
    vec2 offset_bufpos = v_BufPos + (u_TileOffsets / u_WorldSize.zz);
    // base coordinates for the charmap tile of the "nearest" (left/down) vertex.
    vec2 bufTileCoords = floor(offset_bufpos);

    // "raw" offset, expressed as 0.0..1.0, for the offset position of the current
    // fragment
    // -- need to flip the y coords
    vec2 rawUvOffsets = vec2(offset_bufpos.x - bufTileCoords.x, 1.0 - (offset_bufpos.y - bufTileCoords.y));

    vec4 texData;
    if (bufTileCoords.x >= 0.0 && bufTileCoords.x < u_WorldSize.x && bufTileCoords.y >= 0.0 && bufTileCoords.y < u_WorldSize.y) {
        int bufIdx = int((bufTileCoords.y * u_WorldSize.x) + bufTileCoords.x);
        vec4 entry = u_Data[bufIdx].data;

        vec2 uvCoords = (entry.xy + rawUvOffsets) / u_TilesheetSize.xy;
        texData = texture(t_TileSheet, uvCoords);
    } else {
        // if we're here it means the buftilecoords are outside the buffer, so let's just show black
        texData = vec4(0.0,0.0,0.0,1.0);
    }

    Target0 = texData;
}
