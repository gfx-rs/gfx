use crate::native::VertexAttribFunction;
use hal::{
    format::Format,
    image::{self as i, Extent},
    pso,
};

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

pub const COMPRESSED_RGB_S3TC_DXT1_EXT: u32 = 0x83F0;
pub const COMPRESSED_RGBA_S3TC_DXT1_EXT: u32 = 0x83F1;
pub const COMPRESSED_SRGB_S3TC_DXT1_EXT: u32 = 0x8C4C;
pub const COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT: u32 = 0x8C4D;
pub const COMPRESSED_RGBA_S3TC_DXT3_EXT: u32 = 0x83F2;
pub const COMPRESSED_SRGB_ALPHA_S3TC_DXT3_EXT: u32 = 0x8C4E;
pub const COMPRESSED_RGBA_S3TC_DXT5_EXT: u32 = 0x83F3;
pub const COMPRESSED_SRGB_ALPHA_S3TC_DXT5_EXT: u32 = 0x8C4F;

#[derive(Clone, Debug)]
pub struct CompressedFormatInfo {
    pub internal_format: u32,
    pub compressed_block_width: u32,
    pub compressed_block_height: u32,
    pub compressed_block_depth: u32,
    pub compressed_block_size: u32,
    pub component_count: u32,
    pub srgb: bool,
}

impl CompressedFormatInfo {
    const fn new(
        internal_format: u32,
        compressed_block_width: u32,
        compressed_block_height: u32,
        compressed_block_depth: u32,
        compressed_block_size: u32,
        component_count: u32,
        srgb: bool,
    ) -> Self {
        Self {
            internal_format,
            compressed_block_width,
            compressed_block_height,
            compressed_block_depth,
            compressed_block_size,
            component_count,
            srgb,
        }
    }

    pub const fn compute_compressed_image_size(&self, size: Extent) -> u32 {
        let num_blocks_wide =
            (size.width + self.compressed_block_width - 1) / self.compressed_block_width;
        let num_blocks_high =
            (size.height + self.compressed_block_height - 1) / self.compressed_block_height;
        num_blocks_wide * num_blocks_high * (self.compressed_block_size / 8) * size.depth
    }
}

pub const fn compressed_format_info(f: u32) -> Option<CompressedFormatInfo> {
    Some(match f {
        COMPRESSED_RGB_S3TC_DXT1_EXT => CompressedFormatInfo::new(f, 4, 4, 1, 64, 3, false),
        COMPRESSED_RGBA_S3TC_DXT1_EXT => CompressedFormatInfo::new(f, 4, 4, 1, 64, 4, false),
        COMPRESSED_SRGB_S3TC_DXT1_EXT => CompressedFormatInfo::new(f, 4, 4, 1, 64, 3, true),
        COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT => CompressedFormatInfo::new(f, 4, 4, 1, 64, 4, true),
        COMPRESSED_RGBA_S3TC_DXT3_EXT => CompressedFormatInfo::new(f, 4, 4, 1, 128, 4, false),
        COMPRESSED_SRGB_ALPHA_S3TC_DXT3_EXT => CompressedFormatInfo::new(f, 4, 4, 1, 128, 4, true),
        COMPRESSED_RGBA_S3TC_DXT5_EXT => CompressedFormatInfo::new(f, 4, 4, 1, 128, 4, false),
        COMPRESSED_SRGB_ALPHA_S3TC_DXT5_EXT => CompressedFormatInfo::new(f, 4, 4, 1, 128, 4, true),
        _ => return None,
    })
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
        Bgra8Unorm => FormatDescription::new(glow::BGRA, glow::BGRA, glow::UNSIGNED_BYTE, 4, Float),
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
        R16Sfloat => FormatDescription::new(glow::R16F, glow::RED, glow::HALF_FLOAT, 1, Float),
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
        Rg16Sfloat => FormatDescription::new(glow::RG16F, glow::RG, glow::HALF_FLOAT, 2, Float),
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
            FormatDescription::new(glow::RGBA16F, glow::RGBA, glow::HALF_FLOAT, 4, Float)
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
        Bc1RgbUnorm => FormatDescription::new(
            COMPRESSED_RGB_S3TC_DXT1_EXT,
            glow::RGB,
            glow::INVALID_ENUM,
            3,
            Float,
        ),
        Bc1RgbSrgb => FormatDescription::new(
            COMPRESSED_SRGB_S3TC_DXT1_EXT,
            glow::RGB,
            glow::INVALID_ENUM,
            3,
            Float,
        ),
        Bc1RgbaUnorm => FormatDescription::new(
            COMPRESSED_RGBA_S3TC_DXT1_EXT,
            glow::RGBA,
            glow::INVALID_ENUM,
            4,
            Float,
        ),
        Bc1RgbaSrgb => FormatDescription::new(
            COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT,
            glow::RGBA,
            glow::INVALID_ENUM,
            4,
            Float,
        ),
        Bc2Unorm => FormatDescription::new(
            COMPRESSED_RGBA_S3TC_DXT3_EXT,
            glow::RGBA,
            glow::INVALID_ENUM,
            4,
            Float,
        ),
        Bc2Srgb => FormatDescription::new(
            COMPRESSED_SRGB_ALPHA_S3TC_DXT3_EXT,
            glow::RGBA,
            glow::INVALID_ENUM,
            4,
            Float,
        ),
        Bc3Unorm => FormatDescription::new(
            COMPRESSED_RGBA_S3TC_DXT5_EXT,
            glow::RGBA,
            glow::INVALID_ENUM,
            4,
            Float,
        ),
        Bc3Srgb => FormatDescription::new(
            COMPRESSED_SRGB_ALPHA_S3TC_DXT5_EXT,
            glow::RGBA,
            glow::INVALID_ENUM,
            4,
            Float,
        ),
        _ => return None,
    })
}

#[cfg(feature = "cross")]
pub fn map_naga_stage_to_cross(stage: naga::ShaderStage) -> spirv_cross::spirv::ExecutionModel {
    use spirv_cross::spirv::ExecutionModel as Em;
    match stage {
        naga::ShaderStage::Vertex => Em::Vertex,
        naga::ShaderStage::Fragment => Em::Fragment,
        naga::ShaderStage::Compute => Em::GlCompute,
    }
}
