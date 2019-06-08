use crate::hal::format::Format;
use crate::hal::{buffer, image as i, Primitive};
use crate::native::VertexAttribFunction;

/*
pub fn _image_kind_to_gl(kind: i::Kind) -> t::GLenum {
    match kind {
        i::Kind::D1(_) => glow::TEXTURE_1D,
        i::Kind::D1Array(_, _) => glow::TEXTURE_1D_ARRAY,
        i::Kind::D2(_, _, i::AaMode::Single) => glow::TEXTURE_2D,
        i::Kind::D2(_, _, _) => glow::TEXTURE_2D_MULTISAMPLE,
        i::Kind::D2Array(_, _, _, i::AaMode::Single) => glow::TEXTURE_2D_ARRAY,
        i::Kind::D2Array(_, _, _, _) => glow::TEXTURE_2D_MULTISAMPLE_ARRAY,
        i::Kind::D3(_, _, _) => glow::TEXTURE_3D,
        i::Kind::Cube(_) => glow::TEXTURE_CUBE_MAP,
        i::Kind::CubeArray(_, _) => glow::TEXTURE_CUBE_MAP_ARRAY,
    }
}*/

pub fn filter_to_gl(mag: i::Filter, min: i::Filter, mip: i::Filter) -> (u32, u32) {
    use crate::hal::image::Filter::*;

    let mag_filter = match mag {
        Nearest => glow::NEAREST,
        Linear => glow::LINEAR,
    };

    let min_filter = match (min, mip) {
        (Nearest, Nearest) => glow::NEAREST_MIPMAP_NEAREST,
        (Nearest, Linear) => glow::NEAREST_MIPMAP_LINEAR,
        (Linear, Nearest) => glow::LINEAR_MIPMAP_NEAREST,
        (Linear, Linear) => glow::LINEAR_MIPMAP_LINEAR,
    };

    (min_filter, mag_filter)
}

pub fn wrap_to_gl(w: i::WrapMode) -> u32 {
    match w {
        i::WrapMode::Tile => glow::REPEAT,
        i::WrapMode::Mirror => glow::MIRRORED_REPEAT,
        i::WrapMode::Clamp => glow::CLAMP_TO_EDGE,
        i::WrapMode::Border => glow::CLAMP_TO_BORDER,
    }
}

pub fn buffer_usage_to_gl_target(usage: buffer::Usage) -> Option<u32> {
    use self::buffer::Usage;
    match usage & (Usage::UNIFORM | Usage::INDEX | Usage::VERTEX | Usage::INDIRECT) {
        Usage::UNIFORM => Some(glow::UNIFORM_BUFFER),
        Usage::INDEX => Some(glow::ELEMENT_ARRAY_BUFFER),
        Usage::VERTEX => Some(glow::ARRAY_BUFFER),
        Usage::INDIRECT => unimplemented!(),
        _ => None,
    }
}

pub fn primitive_to_gl_primitive(primitive: Primitive) -> u32 {
    match primitive {
        Primitive::PointList => glow::POINTS,
        Primitive::LineList => glow::LINES,
        Primitive::LineStrip => glow::LINE_STRIP,
        Primitive::TriangleList => glow::TRIANGLES,
        Primitive::TriangleStrip => glow::TRIANGLE_STRIP,
        Primitive::LineListAdjacency => glow::LINES_ADJACENCY,
        Primitive::LineStripAdjacency => glow::LINE_STRIP_ADJACENCY,
        Primitive::TriangleListAdjacency => glow::TRIANGLES_ADJACENCY,
        Primitive::TriangleStripAdjacency => glow::TRIANGLE_STRIP_ADJACENCY,
        Primitive::PatchList(_) => glow::PATCHES,
    }
}

pub fn format_to_gl_format(
    format: Format,
) -> Option<(i32, u32, VertexAttribFunction)> {
    use crate::hal::format::Format::*;
    use crate::native::VertexAttribFunction::*;
    let _ = Double; //mark as used
                    // TODO: Add more formats and error handling for `None`
    let format = match format {
        R8Uint => (1, glow::UNSIGNED_BYTE, Integer),
        R8Sint => (1, glow::BYTE, Integer),
        Rg8Uint => (2, glow::UNSIGNED_BYTE, Integer),
        Rg8Sint => (2, glow::BYTE, Integer),
        Rgba8Uint => (4, glow::UNSIGNED_BYTE, Integer),
        Rgba8Sint => (4, glow::BYTE, Integer),
        R16Uint => (1, glow::UNSIGNED_SHORT, Integer),
        R16Sint => (1, glow::SHORT, Integer),
        R16Sfloat => (1, glow::HALF_FLOAT, Float),
        Rg16Uint => (2, glow::UNSIGNED_SHORT, Integer),
        Rg16Sint => (2, glow::SHORT, Integer),
        Rg16Sfloat => (2, glow::HALF_FLOAT, Float),
        Rgba16Uint => (4, glow::UNSIGNED_SHORT, Integer),
        Rgba16Sint => (4, glow::SHORT, Integer),
        Rgba16Sfloat => (4, glow::HALF_FLOAT, Float),
        R32Uint => (1, glow::UNSIGNED_INT, Integer),
        R32Sint => (1, glow::INT, Integer),
        R32Sfloat => (1, glow::FLOAT, Float),
        Rg32Uint => (2, glow::UNSIGNED_INT, Integer),
        Rg32Sint => (2, glow::INT, Integer),
        Rg32Sfloat => (2, glow::FLOAT, Float),
        Rgb32Uint => (3, glow::UNSIGNED_INT, Integer),
        Rgb32Sint => (3, glow::INT, Integer),
        Rgb32Sfloat => (3, glow::FLOAT, Float),
        Rgba32Uint => (4, glow::UNSIGNED_INT, Integer),
        Rgba32Sint => (4, glow::INT, Integer),
        Rgba32Sfloat => (4, glow::FLOAT, Float),

        _ => return None,
    };

    Some(format)
}
