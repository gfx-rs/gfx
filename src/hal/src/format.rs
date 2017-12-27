//! Universal format specification.
//! Applicable to textures, views, and vertex buffers.

/// Description of the bits distribution of a format.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormatBits {
    /// Total number of bits.
    ///
    /// * Depth/Stencil formats are opaque formats, where the total number of bits is unknown.
    ///   A dummy value is used for these formats instead (sum of depth and stencil bits).
    ///   For copy operations, the number of bits of the corresonding aspect should be used.
    /// * The total number can be larger than the sum of `color`, `alpha`, `depth` and `stencil`
    ///   for packed formats.
    /// * For compressed formats, this denotes the number of bits per block.
    pub total: u16,
    /// Number of color bits (summed for R/G/B).
    ///
    /// For compressed formats, this value is 0.
    pub color: u8,
    /// Number of alpha bits.
    ///
    /// For compressed formats, this value is 0.
    pub alpha: u8,
    /// Number of depth bits
    pub depth: u8,
    /// Number of stencil bits
    pub stencil: u8,
}

/// Format bits configuration with no bits assigned.
pub const BITS_ZERO: FormatBits = FormatBits {
    total: 0,
    color: 0,
    alpha: 0,
    depth: 0,
    stencil: 0,
};

/// Source channel in a swizzle configuration. Some may redirect onto
/// different physical channels, some may be hardcoded to 0 or 1.
#[allow(missing_docs)]
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Component {
    Zero,
    One,
    R,
    G,
    B,
    A,
}

/// Channel swizzle configuration for the resource views.
/// Note: It's not currently mirrored at compile-time,
/// thus providing less safety and convenience.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Swizzle(pub Component, pub Component, pub Component, pub Component);

impl Swizzle {
    /// A trivially non-swizzling configuration.
    pub const NO: Swizzle = Swizzle(Component::R, Component::G, Component::B, Component::A);
}

impl Default for Swizzle {
    fn default() -> Self {
        Self::NO
    }
}

/// Format properties of the physical device.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Properties {
    ///
    pub linear_tiling: ImageFeature,
    ///
    pub optimal_tiling: ImageFeature,
    ///
    pub buffer_features: BufferFeature,
}

bitflags!(
    /// Image feature flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct ImageFeature: u16 {
        /// Image view can be sampled.
        const SAMPLED = 0x1;
        /// Image view can be used as storage image.
        const STORAGE = 0x2;
        /// Image view can be used as storage image (with atomics).
        const STORAGE_ATOMIC = 0x4;
        /// Image view can be used as color and input attachment.
        const COLOR_ATTACHMENT = 0x8;
        /// Image view can be used as color (with blending) and input attachment.
        const COLOR_ATTACHMENT_BLEND = 0x10;
        /// Image view can be used as depth-stencil and input attachment.
        const DEPTH_STENCIL_ATTACHMENT = 0x20;
        /// Image can be used as source for blit commands.
        const BLIT_SRC = 0x40;
        /// Image can be used as destination for blit commands.
        const BLIT_DST = 0x80;
        /// Image can be sampled with a (mipmap) linear sampler or as blit source
        /// with linear sampling.
        /// Requires `SAMPLED` or `BLIT_SRC` flag.
        const SAMPLED_LINEAR = 0x100;
    }
);

bitflags!(
    /// Buffer feature flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct BufferFeature: u16 {
        /// Buffer view can be used as uniform texel buffer.
        const UNIFORM_TEXEL = 0x1;
        /// Buffer view can be used as storage texel buffer.
        const STORAGE_TEXEL = 0x2;
        /// Buffer view can be used as storage texel buffer (with atomics).
        const STORAGE_TEXEL_ATOMIC = 0x4;
        /// Image view can be used as vertex buffer.
        const VERTEX = 0x8;
    }
);

/// Type of a surface channel. This is how we interpret the
/// storage allocated with `SurfaceType`.
#[allow(missing_docs)]
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ChannelType {
    /// Unsigned normalized.
    Unorm,
    /// Signed normalized.
    Inorm,
    /// Unsigned integer.
    Uint,
    /// Signed integer.
    Int,
    /// Unsigned floating-point.
    Ufloat,
    /// Signed floating-point.
    Float,
    /// Unsigned scaled integer.
    Uscaled,
    /// Signed scaled integer.
    Iscaled,
    /// Unsigned normalized, SRGB non-linear encoded.
    Srgb,
}

macro_rules! surface_types {
    { $($name:ident { $total:expr $( ,$component:ident : $bits:expr )*} ,)* } => {
        /// Type of the allocated texture surface. It is supposed to only
        /// carry information about the number of bits per each channel.
        /// The actual types are up to the views to decide and interpret.
        /// The actual components are up to the swizzle to define.
        #[repr(u8)]
        #[allow(missing_docs, non_camel_case_types)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        pub enum SurfaceType {
            $( $name, )*
        }

        impl SurfaceType {
            /// Return the total number of bits for this format.
            pub fn describe_bits(&self) -> FormatBits {
                match *self {
                    $( SurfaceType::$name => FormatBits {
                        total: $total,
                        $( $component: $bits, )*
                        .. BITS_ZERO
                    }, )*
                }
            }
        }
    }
}

surface_types! {
    R4_G4 { 8, color: 8 },
    R4_G4_B4_A4 { 32, color: 24, alpha: 4 },
    B4_G4_R4_A4 { 32, color: 24, alpha: 4 },
    R5_G6_B5 { 16, color: 16 },
    B5_G6_R5 { 16, color: 16 },
    R5_G5_B5_A1 { 16, color: 15, alpha: 1 },
    B5_G5_R5_A1 { 16, color: 15, alpha: 1 },
    A1_R5_G5_B5 { 16, color: 15, alpha: 1 },
    R8 { 8, color: 8 },
    R8_G8 { 16, color: 16 },
    R8_G8_B8 { 24, color: 24 },
    B8_G8_R8 { 24, color: 24 },
    R8_G8_B8_A8 { 32, color: 24, alpha: 8 },
    B8_G8_R8_A8 { 32, color: 24, alpha: 8 },
    A8_B8_G8_R8 { 32, color: 24, alpha: 8 },
    A2_R10_G10_B10 { 32, color: 30, alpha: 2 },
    A2_B10_G10_R10 { 32, color: 30, alpha: 2 },
    R16 { 16, color: 16 },
    R16_G16 { 32, color: 32 },
    R16_G16_B16 { 48, color: 48 },
    R16_G16_B16_A16 { 48, color: 48, alpha: 16 },
    R32 { 32, color: 32 },
    R32_G32 { 64, color: 64 },
    R32_G32_B32 { 96, color: 96 },
    R32_G32_B32_A32 { 128, color: 96, alpha: 32 },
    R64 { 64, color: 64 },
    R64_G64 { 128, color: 128 },
    R64_G64_B64 { 192, color: 192 },
    R64_G64_B64_A64 { 256, color: 192, alpha: 64 },
    B10_G11_R11 { 32, color: 32 },
    E5_B9_G9_R9 { 32, color: 27 }, // 32-bit packed format
    D16 { 16, depth: 16 },
    X8D24 { 32, depth: 24 },
    D32 { 32, depth: 32 },
    S8 { 8, stencil: 8 },
    D16_S8 { 24, depth: 16, stencil: 8 },
    D24_S8 { 32, depth: 24, stencil: 8 },
    D32_S8 { 40, depth: 32, stencil: 8 },
    BC1_RGB { 64 },
    BC1_RGBA { 64 },
    BC2 { 128 },
    BC3 { 128 },
    BC4 { 64 },
    BC5 { 128 },
    BC6 { 128 },
    BC7 { 128 },
    ETC2_R8_G8_B8 { 64 },
    ETC2_R8_G8_B8_A1 { 64 },
    ETC2_R8_G8_B8_A8 { 128 },
    EAC_R11 { 64 },
    EAC_R11_G11 { 128 },
    ASTC_4x4 { 128 },
    ASTC_5x4 { 128 },
    ASTC_5x5 { 128 },
    ASTC_6x5 { 128 },
    ASTC_6x6 { 128 },
    ASTC_8x5 { 128 },
    ASTC_8x6 { 128 },
    ASTC_8x8 { 128 },
    ASTC_10x5 { 128 },
    ASTC_10x6 { 128 },
    ASTC_10x8 { 128 },
    ASTC_10x10 { 128 },
    ASTC_12x10 { 128 },
    ASTC_12x12 { 128 },
}

/// Gneric run-time base format.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BaseFormat(pub SurfaceType, pub ChannelType);

macro_rules! formats {
    { $($name:ident = ($surface:ident, $channel:ident),)* } => {
        ///
        #[allow(missing_docs)]
        #[repr(u8)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        pub enum Format {
            Undefined = 0,
            $( $name, )*
        }

        impl From<Format> for Option<BaseFormat> {
            fn from(format: Format) -> Self {
                match format {
                    Format::Undefined => None,
                    $(
                        Format::$name => Some(BaseFormat(SurfaceType::$surface, ChannelType::$channel)),
                    )*
                }
            }
        }

        // TODO: test for equality hal <-> vk formats
    }
}

formats! {
    Rg4Unorm = (R4_G4, Unorm),
    Rgba4Unorm = (R4_G4_B4_A4, Unorm),
    Bgra4Unorm = (B4_G4_R4_A4, Unorm),
    R5g6b5Unorm = (R5_G6_B5, Unorm),
    B5g6r5Unorm = (B5_G6_R5, Unorm),
    R5g5b5a1Unorm = (R5_G5_B5_A1, Unorm),
    B5g5r5a1Unorm = (B5_G5_R5_A1, Unorm),
    A1r5g5b5Unorm = (A1_R5_G5_B5, Unorm),
    R8Unorm = (R8, Unorm),
    R8Inorm = (R8, Inorm),
    R8Uscaled = (R8, Uscaled),
    R8Iscaled = (R8, Iscaled),
    R8Uint = (R8, Uint),
    R8Int = (R8, Int),
    R8Srgb = (R8, Srgb),
    Rg8Unorm = (R8_G8, Unorm),
    Rg8Inorm = (R8_G8, Inorm),
    Rg8Uscaled = (R8_G8, Uscaled),
    Rg8Iscaled = (R8_G8, Iscaled),
    Rg8Uint = (R8_G8, Uint),
    Rg8Int = (R8_G8, Int),
    Rg8Srgb = (R8_G8, Srgb),
    Rgb8Unorm = (R8_G8_B8, Unorm),
    Rgb8Inorm = (R8_G8_B8, Inorm),
    Rgb8Uscaled = (R8_G8_B8, Uscaled),
    Rgb8Iscaled = (R8_G8_B8, Iscaled),
    Rgb8Uint = (R8_G8_B8, Uint),
    Rgb8Int = (R8_G8_B8, Int),
    Rgb8Srgb = (R8_G8_B8, Srgb),
    Bgr8Unorm = (B8_G8_R8, Unorm),
    Bgr8Inorm = (B8_G8_R8, Inorm),
    Bgr8Uscaled = (B8_G8_R8, Uscaled),
    Bgr8Iscaled = (B8_G8_R8, Iscaled),
    Bgr8Uint = (B8_G8_R8, Uint),
    Bgr8Int = (B8_G8_R8, Int),
    Bgr8Srgb = (B8_G8_R8, Srgb),
    Rgba8Unorm = (R8_G8_B8_A8, Unorm),
    Rgba8Inorm = (R8_G8_B8_A8, Inorm),
    Rgba8Uscaled = (R8_G8_B8_A8, Uscaled),
    Rgba8Iscaled = (R8_G8_B8_A8, Iscaled),
    Rgba8Uint = (R8_G8_B8_A8, Uint),
    Rgba8Int = (R8_G8_B8_A8, Int),
    Rgba8Srgb = (R8_G8_B8_A8, Srgb),
    Bgra8Unorm = (B8_G8_R8_A8, Unorm),
    Bgra8Inorm = (B8_G8_R8_A8, Inorm),
    Bgra8Uscaled = (B8_G8_R8_A8, Uscaled),
    Bgra8Iscaled = (B8_G8_R8_A8, Iscaled),
    Bgra8Uint = (B8_G8_R8_A8, Uint),
    Bgra8Int = (B8_G8_R8_A8, Int),
    Bgra8Srgb = (B8_G8_R8_A8, Srgb),
    Abgr8Unorm = (A8_B8_G8_R8, Unorm),
    Abgr8Inorm = (A8_B8_G8_R8, Inorm),
    Abgr8Uscaled = (A8_B8_G8_R8, Uscaled),
    Abgr8Iscaled = (A8_B8_G8_R8, Iscaled),
    Abgr8Uint = (A8_B8_G8_R8, Uint),
    Abgr8Int = (A8_B8_G8_R8, Int),
    Abgr8Srgb = (A8_B8_G8_R8, Srgb),
    A2r10g10b10Unorm = (A2_R10_G10_B10, Unorm),
    A2r10g10b10Inorm = (A2_R10_G10_B10, Inorm),
    A2r10g10b10Uscaled = (A2_R10_G10_B10, Uscaled),
    A2r10g10b10Iscaled = (A2_R10_G10_B10, Iscaled),
    A2r10g10b10Uint = (A2_R10_G10_B10, Uint),
    A2r10g10b10Int = (A2_R10_G10_B10, Int),
    A2b10g10r10Unorm = (A2_B10_G10_R10, Unorm),
    A2b10g10r10Inorm = (A2_B10_G10_R10, Inorm),
    A2b10g10r10Uscaled = (A2_B10_G10_R10, Uscaled),
    A2b10g10r10Iscaled = (A2_B10_G10_R10, Iscaled),
    A2b10g10r10Uint = (A2_B10_G10_R10, Uint),
    A2b10g10r10Int = (A2_B10_G10_R10, Int),
    R16Unorm = (R16, Unorm),
    R16Inorm = (R16, Inorm),
    R16Uscaled = (R16, Uscaled),
    R16Iscaled = (R16, Iscaled),
    R16Uint = (R16, Uint),
    R16Int = (R16, Int),
    R16Float = (R16, Float),
    Rg16Unorm = (R16_G16, Unorm),
    Rg16Inorm = (R16_G16, Inorm),
    Rg16Uscaled = (R16_G16, Uscaled),
    Rg16Iscaled = (R16_G16, Iscaled),
    Rg16Uint = (R16_G16, Uint),
    Rg16Int = (R16_G16, Int),
    Rg16Float = (R16_G16, Float),
    Rgb16Unorm = (R16_G16_B16, Unorm),
    Rgb16Inorm = (R16_G16_B16, Inorm),
    Rgb16Uscaled = (R16_G16_B16, Uscaled),
    Rgb16Iscaled = (R16_G16_B16, Iscaled),
    Rgb16Uint = (R16_G16_B16, Uint),
    Rgb16Int = (R16_G16_B16, Int),
    Rgb16Float = (R16_G16_B16, Float),
    Rgba16Unorm = (R16_G16_B16_A16, Unorm),
    Rgba16Inorm = (R16_G16_B16_A16, Inorm),
    Rgba16Uscaled = (R16_G16_B16_A16, Uscaled),
    Rgba16Iscaled = (R16_G16_B16_A16, Iscaled),
    Rgba16Uint = (R16_G16_B16_A16, Uint),
    Rgba16Int = (R16_G16_B16_A16, Int),
    Rgba16Float = (R16_G16_B16_A16, Float),
    R32Uint = (R32, Uint),
    R32Int = (R32, Int),
    R32Float = (R32, Float),
    Rg32Uint = (R32_G32, Uint),
    Rg32Int = (R32_G32, Int),
    Rg32Float = (R32_G32, Float),
    Rgb32Uint = (R32_G32_B32, Uint),
    Rgb32Int = (R32_G32_B32, Int),
    Rgb32Float = (R32_G32_B32, Float),
    Rgba32Uint = (R32_G32_B32_A32, Uint),
    Rgba32Int = (R32_G32_B32_A32, Int),
    Rgba32Float = (R32_G32_B32_A32, Float),
    R64Uint = (R64, Uint),
    R64Int = (R64, Int),
    R64Float = (R64, Float),
    Rg64Uint = (R64_G64, Uint),
    Rg64Int = (R64_G64, Int),
    Rg64Float = (R64_G64, Float),
    Rgb64Uint = (R64_G64_B64, Uint),
    Rgb64Int = (R64_G64_B64, Int),
    Rgb64Float = (R64_G64_B64, Float),
    Rgba64Uint = (R64_G64_B64_A64, Uint),
    Rgba64Int = (R64_G64_B64_A64, Int),
    Rgba64Float = (R64_G64_B64_A64, Float),
    B10g11r11Ufloat = (B10_G11_R11, Ufloat),
    E5b9g9r9Ufloat = (E5_B9_G9_R9, Ufloat),
    D16Unorm = (D16, Unorm),
    X8D24Unorm = (X8D24, Unorm),
    D32Float = (D32, Float),
    S8Uint = (S8, Uint),
    D16UnormS8Uint = (D16_S8, Unorm),
    D24UnormS8Uint = (D24_S8, Unorm),
    D32FloatS8Uint = (D32_S8, Float),
    Bc1RgbUnorm = (BC1_RGB, Unorm),
    Bc1RgbSrgb = (BC1_RGB, Srgb),
    Bc1RgbaUnorm = (BC1_RGBA, Unorm),
    Bc1RgbaSrgb = (BC1_RGBA, Srgb),
    Bc2Unorm = (BC2, Unorm),
    Bc2Srgb = (BC2, Srgb),
    Bc3Unorm = (BC3, Unorm),
    Bc3Srgb = (BC3, Srgb),
    Bc4Unorm = (BC4, Unorm),
    Bc4Inorm = (BC4, Inorm),
    Bc5Unorm = (BC5, Unorm),
    Bc5Inorm = (BC5, Inorm),
    Bc6hUfloat = (BC6, Ufloat),
    Bc6hFloat = (BC6, Float),
    Bc7Unorm = (BC7, Unorm),
    Bc7Srgb = (BC7, Srgb),
    Etc2R8g8b8Unorm = (ETC2_R8_G8_B8, Unorm),
    Etc2R8g8b8Srgb = (ETC2_R8_G8_B8, Srgb),
    Etc2R8g8b8a1Unorm = (ETC2_R8_G8_B8_A1, Unorm),
    Etc2R8g8b8a1Srgb = (ETC2_R8_G8_B8_A1, Srgb),
    Etc2R8g8b8a8Unorm = (ETC2_R8_G8_B8_A8, Unorm),
    Etc2R8g8b8a8Srgb = (ETC2_R8_G8_B8_A8, Srgb),
    EacR11Unorm = (EAC_R11, Unorm),
    EacR11Inorm = (EAC_R11, Unorm),
    EacR11g11Unorm = (EAC_R11_G11, Unorm),
    EacR11g11Inorm = (EAC_R11_G11, Inorm),
    Astc4x4Unorm = (ASTC_4x4, Unorm),
    Astc4x4Srgb = (ASTC_4x4, Srgb),
    Astc5x4Unorm = (ASTC_5x4, Unorm),
    Astc5x4Srgb = (ASTC_5x4, Srgb),
    Astc5x5Unorm = (ASTC_5x5, Unorm),
    Astc5x5Srgb = (ASTC_5x5, Srgb),
    Astc6x5Unorm = (ASTC_6x5, Unorm),
    Astc6x5Srgb = (ASTC_6x5, Srgb),
    Astc6x6Unorm = (ASTC_6x6, Unorm),
    Astc6x6Srgb = (ASTC_6x6, Srgb),
    Astc8x5Unorm = (ASTC_8x5, Unorm),
    Astc8x5Srgb = (ASTC_8x5, Srgb),
    Astc8x6Unorm = (ASTC_8x6, Unorm),
    Astc8x6Srgb = (ASTC_8x6, Srgb),
    Astc8x8Unorm = (ASTC_8x8, Unorm),
    Astc8x8Srgb = (ASTC_8x8, Srgb),
    Astc10x5Unorm = (ASTC_10x5, Unorm),
    Astc10x5Srgb = (ASTC_10x5, Srgb),
    Astc10x6Unorm = (ASTC_10x6, Unorm),
    Astc10x6Srgb = (ASTC_10x6, Srgb),
    Astc10x8Unorm = (ASTC_10x8, Unorm),
    Astc10x8Srgb = (ASTC_10x8, Srgb),
    Astc10x10Unorm = (ASTC_10x10, Unorm),
    Astc10x10Srgb = (ASTC_10x10, Srgb),
    Astc12x10Unorm = (ASTC_12x10, Unorm),
    Astc12x10Srgb = (ASTC_12x10, Srgb),
    Astc12x12Unorm = (ASTC_12x12, Unorm),
    Astc12x12Srgb = (ASTC_12x12, Srgb),
}
