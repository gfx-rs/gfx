use gl::{self, types as t};
use hal::{buffer, image as i, Primitive};
use hal::format::{Format};

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
}

pub fn filter_to_gl(f: i::FilterMethod) -> (t::GLenum, t::GLenum) {
    match f {
        i::FilterMethod::Scale => (gl::NEAREST, gl::NEAREST),
        i::FilterMethod::Mipmap => (gl::NEAREST_MIPMAP_NEAREST, gl::NEAREST),
        i::FilterMethod::Bilinear => (gl::LINEAR, gl::LINEAR),
        i::FilterMethod::Trilinear => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
        i::FilterMethod::Anisotropic(..) => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
    }
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

pub fn format_to_gl_format(format: Format) -> Option<(gl::types::GLint, gl::types::GLenum)> {
    use hal::format::Format::*;
    // TODO: Add more formats and error handling for `None`
    let format = match format {
        R8Uint => (1, gl::UNSIGNED_BYTE),
        R8Int => (1, gl::BYTE),
        Rg8Uint => (2, gl::UNSIGNED_BYTE),
        Rg8Int => (2, gl::BYTE),
        Rgba8Uint => (4, gl::UNSIGNED_BYTE),
        Rgba8Int => (4, gl::BYTE),
        R16Uint => (1, gl::UNSIGNED_SHORT),
        R16Int => (1, gl::SHORT),
        R16Float => (1, gl::HALF_FLOAT),
        Rg16Uint => (2, gl::UNSIGNED_SHORT),
        Rg16Int => (2, gl::SHORT),
        Rg16Float => (2, gl::HALF_FLOAT),
        Rgba16Uint => (4, gl::UNSIGNED_SHORT),
        Rgba16Int => (4, gl::SHORT),
        Rgba16Float => (4, gl::HALF_FLOAT),
        R32Uint => (1, gl::UNSIGNED_INT),
        R32Int => (1, gl::INT),
        R32Float => (1, gl::FLOAT),
        Rg32Uint => (2, gl::UNSIGNED_INT),
        Rg32Int => (2, gl::INT),
        Rg32Float => (2, gl::FLOAT),
        Rgb32Uint => (3, gl::UNSIGNED_INT),
        Rgb32Int => (3, gl::INT),
        Rgb32Float => (3, gl::FLOAT),
        Rgba32Uint => (4, gl::UNSIGNED_INT),
        Rgba32Int => (4, gl::INT),
        Rgba32Float => (4, gl::FLOAT),
        
        _ => return None,
    };

    Some(format)
}
