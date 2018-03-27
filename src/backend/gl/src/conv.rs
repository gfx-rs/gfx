use gl::{self, types as t};
use hal::{buffer, image as i, Primitive};
use hal::format::Format;
use native::VertexAttribFunction;

/*
pub fn _image_kind_to_gl(kind: i::Kind) -> t::GLenum {
    match kind {
        i::Kind::D1(_) => gl::TEXTURE_1D,
        i::Kind::D1Array(_, _) => gl::TEXTURE_1D_ARRAY,
        i::Kind::D2(_, _, i::AaMode::Single) => gl::TEXTURE_2D,
        i::Kind::D2(_, _, _) => gl::TEXTURE_2D_MULTISAMPLE,
        i::Kind::D2Array(_, _, _, i::AaMode::Single) => gl::TEXTURE_2D_ARRAY,
        i::Kind::D2Array(_, _, _, _) => gl::TEXTURE_2D_MULTISAMPLE_ARRAY,
        i::Kind::D3(_, _, _) => gl::TEXTURE_3D,
        i::Kind::Cube(_) => gl::TEXTURE_CUBE_MAP,
        i::Kind::CubeArray(_, _) => gl::TEXTURE_CUBE_MAP_ARRAY,
    }
}*/

pub fn filter_to_gl(mag: i::Filter, min: i::Filter, mip: i::Filter) -> (t::GLenum, t::GLenum) {
    use hal::image::Filter::*;

    let mag_filter = match mag {
        Nearest => gl::NEAREST,
        Linear => gl::LINEAR,
    };

    let min_filter = match (min, mip) {
        (Nearest, Nearest) => gl::NEAREST_MIPMAP_NEAREST,
        (Nearest, Linear) => gl::NEAREST_MIPMAP_LINEAR,
        (Linear, Nearest) => gl::LINEAR_MIPMAP_NEAREST,
        (Linear, Linear) => gl::LINEAR_MIPMAP_LINEAR,
    };

    (min_filter, mag_filter)
}

pub fn wrap_to_gl(w: i::WrapMode) -> t::GLenum {
    match w {
        i::WrapMode::Tile   => gl::REPEAT,
        i::WrapMode::Mirror => gl::MIRRORED_REPEAT,
        i::WrapMode::Clamp  => gl::CLAMP_TO_EDGE,
        i::WrapMode::Border => gl::CLAMP_TO_BORDER,
    }
}

pub fn buffer_usage_to_gl_target(usage: buffer::Usage) -> Option<t::GLenum> {
    use self::buffer::Usage;
    match usage & (Usage::UNIFORM | Usage::INDEX | Usage::VERTEX | Usage::INDIRECT) {
        Usage::UNIFORM => Some(gl::UNIFORM_BUFFER),
        Usage::INDEX => Some(gl::ELEMENT_ARRAY_BUFFER),
        Usage::VERTEX => Some(gl::ARRAY_BUFFER),
        Usage::INDIRECT => unimplemented!(),
        _ => None
    }
}

pub fn primitive_to_gl_primitive(primitive: Primitive) -> t::GLenum {
    match primitive {
        Primitive::PointList => gl::POINTS,
        Primitive::LineList => gl::LINES,
        Primitive::LineStrip => gl::LINE_STRIP,
        Primitive::TriangleList => gl::TRIANGLES,
        Primitive::TriangleStrip => gl::TRIANGLE_STRIP,
        Primitive::LineListAdjacency => gl::LINES_ADJACENCY,
        Primitive::LineStripAdjacency => gl::LINE_STRIP_ADJACENCY,
        Primitive::TriangleListAdjacency => gl::TRIANGLES_ADJACENCY,
        Primitive::TriangleStripAdjacency => gl::TRIANGLE_STRIP_ADJACENCY,
        Primitive::PatchList(_) => gl::PATCHES,
    }
}

pub fn format_to_gl_format(format: Format) -> Option<(gl::types::GLint, gl::types::GLenum, VertexAttribFunction)> {
    use hal::format::Format::*;
    use gl::*;
    use native::VertexAttribFunction::*;
    let _ = Double; //mark as used
    // TODO: Add more formats and error handling for `None`
    let format = match format {
        R8Uint => (1, UNSIGNED_BYTE, Integer),
        R8Int => (1, BYTE, Integer),
        Rg8Uint => (2, UNSIGNED_BYTE, Integer),
        Rg8Int => (2, BYTE, Integer),
        Rgba8Uint => (4, UNSIGNED_BYTE, Integer),
        Rgba8Int => (4, BYTE, Integer),
        R16Uint => (1, UNSIGNED_SHORT, Integer),
        R16Int => (1, SHORT, Integer),
        R16Float => (1, HALF_FLOAT, Float),
        Rg16Uint => (2, UNSIGNED_SHORT, Integer),
        Rg16Int => (2, SHORT, Integer),
        Rg16Float => (2, HALF_FLOAT, Float),
        Rgba16Uint => (4, UNSIGNED_SHORT, Integer),
        Rgba16Int => (4, SHORT, Integer),
        Rgba16Float => (4, HALF_FLOAT, Float),
        R32Uint => (1, UNSIGNED_INT, Integer),
        R32Int => (1, INT, Integer),
        R32Float => (1, FLOAT, Float),
        Rg32Uint => (2, UNSIGNED_INT, Integer),
        Rg32Int => (2, INT, Integer),
        Rg32Float => (2, FLOAT, Float),
        Rgb32Uint => (3, UNSIGNED_INT, Integer),
        Rgb32Int => (3, INT, Integer),
        Rgb32Float => (3, FLOAT, Float),
        Rgba32Uint => (4, UNSIGNED_INT, Integer),
        Rgba32Int => (4, INT, Integer),
        Rgba32Float => (4, FLOAT, Float),

        _ => return None,
    };

    Some(format)
}
