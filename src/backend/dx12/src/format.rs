//! Format related queries for the backend.

use hal::format::{BufferFeature, ImageFeature, Properties, NUM_FORMATS};

///
pub fn query_properties() -> [Properties; NUM_FORMATS] {
    // TODO
    let properties = [
        // Undefined
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg4Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba4Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgra4Unorm
        // TODO: check optional supports
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // R5g6b5Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // B5g6r5Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R5g5b5a1Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // B5g5r5a1Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A1r5g5b5Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R8Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R8Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R8Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R8Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg8Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg8Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg8Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg8Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb8Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb8Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb8Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb8Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgr8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgr8Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgr8Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgr8Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgr8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgr8Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgr8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba8Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba8Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba8Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba8Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgra8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgra8Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgra8Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgra8Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgra8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgra8Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bgra8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Abgr8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Abgr8Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Abgr8Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Abgr8Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Abgr8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Abgr8Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Abgr8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2r10g10b10Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2r10g10b10Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2r10g10b10Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2r10g10b10Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2r10g10b10Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2r10g10b10Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2b10g10r10Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2b10g10r10Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2b10g10r10Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2b10g10r10Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2b10g10r10Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // A2b10g10r10Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R16Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R16Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R16Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R16Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R16Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R16Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R16Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg16Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg16Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg16Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg16Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg16Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg16Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg16Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb16Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb16Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb16Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb16Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb16Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb16Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb16Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba16Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba16Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba16Uscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba16Iscaled
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba16Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba16Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba16Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R32Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R32Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R32Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg32Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg32Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg32Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb32Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb32Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb32Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba32Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba32Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba32Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R64Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R64Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // R64Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg64Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg64Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rg64Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb64Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb64Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgb64Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba64Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba64Int
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Rgba64Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // B10g11r11Ufloat
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // E5b9g9r9Ufloat
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // D16Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::DEPTH_STENCIL_ATTACHMENT,
            buffer_features: BufferFeature::empty(),
        },
        // X8D24Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // D32Float
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // S8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // D16UnormS8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // D24UnormS8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // D32FloatS8Uint
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Bc1RgbUnorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc1RgbSrgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc1RgbaUnorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc1RgbaSrgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc2Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc2Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc3Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc3Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc4Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc4Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc5Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc5Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc6hUfloat
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc6hFloat
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc7Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Bc7Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::SAMPLED | ImageFeature::BLIT_SRC | ImageFeature::SAMPLED_LINEAR,
            buffer_features: BufferFeature::empty(),
        },
        // Etc2R8g8b8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Etc2R8g8b8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Etc2R8g8b8a1Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Etc2R8g8b8a1Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Etc2R8g8b8a8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Etc2R8g8b8a8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // EacR11Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // EacR11Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // EacR11g11Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // EacR11g11Inorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc4x4Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc4x4Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc5x4Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc5x4Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc5x5Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc5x5Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc6x5Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc6x5Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc6x6Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc6x6Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc8x5Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc8x5Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc8x6Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc8x6Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc8x8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc8x8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc10x5Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc10x5Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc10x6Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc10x6Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc10x8Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc10x8Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc10x10Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc10x10Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc12x10Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc12x10Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc12x12Unorm
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
        // Astc12x12Srgb
        Properties {
            linear_tiling: ImageFeature::empty(),
            optimal_tiling: ImageFeature::empty(),
            buffer_features: BufferFeature::empty(),
        },
    ];

    properties
}
