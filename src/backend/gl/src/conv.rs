use crate::native::VertexAttribFunction;
use hal::{format::Format, image as i, pso};

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
    use hal::image::Filter::*;

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
        i::WrapMode::MirrorClamp => glow::MIRROR_CLAMP_TO_EDGE,
    }
}

pub fn input_assember_to_gl_primitive(ia: &pso::InputAssemblerDesc) -> u32 {
    match (ia.primitive, ia.with_adjacency) {
        (pso::Primitive::PointList, false) => glow::POINTS,
        (pso::Primitive::PointList, true) => panic!("Points can't have adjacency info"),
        (pso::Primitive::LineList, false) => glow::LINES,
        (pso::Primitive::LineList, true) => glow::LINES_ADJACENCY,
        (pso::Primitive::LineStrip, false) => glow::LINE_STRIP,
        (pso::Primitive::LineStrip, true) => glow::LINE_STRIP_ADJACENCY,
        (pso::Primitive::TriangleList, false) => glow::TRIANGLES,
        (pso::Primitive::TriangleList, true) => glow::TRIANGLES_ADJACENCY,
        (pso::Primitive::TriangleStrip, false) => glow::TRIANGLE_STRIP,
        (pso::Primitive::TriangleStrip, true) => glow::TRIANGLE_STRIP_ADJACENCY,
        (pso::Primitive::PatchList(_), false) => glow::PATCHES,
        (pso::Primitive::PatchList(_), true) => panic!("Patches can't have adjacency info"),
    }
}

pub struct FormatDescription {
    pub tex_internal: u32,
    pub tex_external: u32,
    pub data_type: u32,
    pub num_components: u8,
    pub va_fun: VertexAttribFunction,
}

impl FormatDescription {
    fn new(
        tex_internal: u32,
        tex_external: u32,
        data_type: u32,
        num_components: u8,
        va_fun: VertexAttribFunction,
    ) -> Self {
        FormatDescription {
            tex_internal,
            tex_external,
            data_type,
            num_components,
            va_fun,
        }
    }
}

pub fn describe_format(format: Format) -> Option<FormatDescription> {
    use crate::native::VertexAttribFunction::*;
    use hal::format::Format::*;
    let _ = Double; //mark as used

    // TODO: Add more formats and error handling for `None`
    Some(match format {
        R8Uint => FormatDescription::new(
            glow::R8UI,
            glow::RED_INTEGER,
            glow::UNSIGNED_BYTE,
            1,
            Integer,
        ),
        R8Sint => FormatDescription::new(glow::R8I, glow::RED_INTEGER, glow::BYTE, 1, Integer),
        R8Unorm => FormatDescription::new(glow::R8, glow::RED, glow::UNSIGNED_BYTE, 1, Float),
        Rg8Uint => FormatDescription::new(
            glow::RG8UI,
            glow::RG_INTEGER,
            glow::UNSIGNED_BYTE,
            2,
            Integer,
        ),
        Rg8Sint => FormatDescription::new(glow::RG8I, glow::RG_INTEGER, glow::BYTE, 2, Integer),
        Rgba8Uint => FormatDescription::new(
            glow::RGBA8UI,
            glow::RGBA_INTEGER,
            glow::UNSIGNED_BYTE,
            4,
            Integer,
        ),
        Rgba8Sint => {
            FormatDescription::new(glow::RGBA8I, glow::RGBA_INTEGER, glow::BYTE, 4, Integer)
        }
        Rgba8Unorm => {
            FormatDescription::new(glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE, 4, Float)
        }
        Rgb8Srgb => FormatDescription::new(glow::SRGB8, glow::RGB, glow::UNSIGNED_BYTE, 3, Float),
        Rgba8Srgb => FormatDescription::new(
            glow::SRGB8_ALPHA8,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            4,
            Float,
        ),
        Bgra8Unorm => {
            FormatDescription::new(glow::RGBA8, glow::BGRA, glow::UNSIGNED_BYTE, 4, Float)
        }
        Bgra8Srgb => FormatDescription::new(
            glow::SRGB8_ALPHA8,
            glow::BGRA,
            glow::UNSIGNED_BYTE,
            4,
            Float,
        ),
        R16Uint => FormatDescription::new(
            glow::R16UI,
            glow::RED_INTEGER,
            glow::UNSIGNED_SHORT,
            1,
            Integer,
        ),
        R16Sint => FormatDescription::new(glow::R16I, glow::RED_INTEGER, glow::SHORT, 1, Integer),
        R16Sfloat => FormatDescription::new(glow::R16, glow::RED, glow::HALF_FLOAT, 1, Float),
        R16Unorm => FormatDescription::new(glow::R16, glow::RED, glow::UNSIGNED_SHORT, 1, Float),
        Rg16Uint => FormatDescription::new(
            glow::RG16UI,
            glow::RG_INTEGER,
            glow::UNSIGNED_SHORT,
            2,
            Integer,
        ),
        Rg16Sint => FormatDescription::new(glow::RG16I, glow::RG_INTEGER, glow::SHORT, 2, Integer),
        Rg16Unorm => FormatDescription::new(glow::RG16, glow::RG, glow::UNSIGNED_SHORT, 2, Float),
        Rg16Sfloat => FormatDescription::new(glow::RG16, glow::RG, glow::HALF_FLOAT, 2, Float),
        Rgba16Uint => FormatDescription::new(
            glow::RGBA16UI,
            glow::RGBA_INTEGER,
            glow::UNSIGNED_SHORT,
            4,
            Integer,
        ),
        Rgba16Sint => {
            FormatDescription::new(glow::RGBA16I, glow::RGBA_INTEGER, glow::SHORT, 4, Integer)
        }
        Rgba16Sfloat => {
            FormatDescription::new(glow::RGBA16, glow::RGBA, glow::HALF_FLOAT, 4, Float)
        }
        Rgba16Unorm => {
            FormatDescription::new(glow::RGBA16, glow::RGBA, glow::UNSIGNED_SHORT, 4, Float)
        }
        R32Uint => FormatDescription::new(
            glow::R32UI,
            glow::RED_INTEGER,
            glow::UNSIGNED_INT,
            1,
            Integer,
        ),
        R32Sint => FormatDescription::new(glow::R32I, glow::RED_INTEGER, glow::INT, 1, Integer),
        R32Sfloat => FormatDescription::new(glow::R32F, glow::RED, glow::FLOAT, 1, Float),
        Rg32Uint => FormatDescription::new(
            glow::RG32UI,
            glow::RG_INTEGER,
            glow::UNSIGNED_INT,
            2,
            Integer,
        ),
        Rg32Sint => FormatDescription::new(glow::R32I, glow::RG_INTEGER, glow::INT, 2, Integer),
        Rg32Sfloat => FormatDescription::new(glow::RG32F, glow::RG, glow::FLOAT, 2, Float),
        Rgb32Uint => FormatDescription::new(
            glow::RGB32UI,
            glow::RGB_INTEGER,
            glow::UNSIGNED_INT,
            3,
            Integer,
        ),
        Rgb32Sint => FormatDescription::new(glow::RGB32I, glow::RGB_INTEGER, glow::INT, 3, Integer),
        Rgb32Sfloat => FormatDescription::new(glow::RGB32F, glow::RGB, glow::FLOAT, 3, Float),
        Rgba32Uint => FormatDescription::new(
            glow::RGBA32UI,
            glow::RGBA_INTEGER,
            glow::UNSIGNED_INT,
            4,
            Integer,
        ),
        Rgba32Sint => {
            FormatDescription::new(glow::RGBA32I, glow::RGBA_INTEGER, glow::INT, 4, Integer)
        }
        Rgba32Sfloat => FormatDescription::new(glow::RGBA32F, glow::RGBA, glow::FLOAT, 4, Float),
        S8Uint => FormatDescription::new(glow::R8, glow::RED, glow::UNSIGNED_BYTE, 1, Integer),
        D16Unorm => FormatDescription::new(
            glow::DEPTH_COMPONENT16,
            glow::DEPTH_COMPONENT,
            glow::UNSIGNED_NORMALIZED,
            1,
            Float,
        ),
        D24UnormS8Uint => FormatDescription::new(
            glow::DEPTH24_STENCIL8,
            glow::DEPTH_STENCIL,
            glow::UNSIGNED_INT,
            2,
            Float,
        ),
        D32Sfloat => FormatDescription::new(
            glow::DEPTH_COMPONENT32F,
            glow::DEPTH_COMPONENT,
            glow::FLOAT,
            1,
            Float,
        ),
        D32SfloatS8Uint => FormatDescription::new(
            glow::DEPTH32F_STENCIL8,
            glow::DEPTH_STENCIL,
            glow::UNSIGNED_INT,
            1,
            Float,
        ),
        X8D24Unorm => FormatDescription::new(
            glow::DEPTH_COMPONENT24,
            glow::DEPTH_STENCIL,
            glow::UNSIGNED_NORMALIZED,
            2,
            Float,
        ),

        _ => return None,
    })
}
