//! Texture creation and modification.
//!
//! "Texture" is an overloaded term. In gfx-rs, a texture consists of two
//! separate pieces of information: an image storage description (which is
//! immutable for a single texture object), and image data. To actually use a
//! texture, a "sampler" is needed, which provides a way of accessing the
//! image data.  Image data consists of an array of "texture elements", or
//! texels.

use std::error::Error;
use std::fmt;
use std::ops::Range;

use format;
use pso::Comparison;


/// Image layer.
pub type Layer = u16;
/// Image mipmap level.
pub type Level = u8;
/// Maximum accessible mipmap level of a image.
pub const MAX_LEVEL: Level = 15;

///
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Offset {
    ///
    pub x: i32,
    ///
    pub y: i32,
    ///
    pub z: i32,
}

/// Pure texture object creation error.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CreationError {
    /// The format is not supported by the device.
    Format(format::Format),
    /// The kind doesn't support a particular operation.
    Kind,
    /// Failed to map a given multisampled kind to the device.
    Samples(AaMode),
    /// Unsupported size in one of the dimensions.
    Size(Size),
    /// The given data has a different size than the target texture slice.
    Data(usize),
    /// The mentioned usage mode is not supported
    Usage(Usage),
}

impl fmt::Display for CreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CreationError::Format(format) => write!(f, "{}: {:?}", self.description(), format),
            CreationError::Samples(aa) => write!(f, "{}: {:?}", self.description(), aa),
            CreationError::Size(size) => write!(f, "{}: {}", self.description(), size),
            CreationError::Data(data) => write!(f, "{}: {}", self.description(), data),
            CreationError::Usage(usage) => write!(f, "{}: {:?}", self.description(), usage),
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl Error for CreationError {
    fn description(&self) -> &str {
        match *self {
            CreationError::Format(..) => "Failed to map a given format to the device",
            CreationError::Kind => "The kind doesn't support a particular operation",
            CreationError::Samples(_) => "Failed to map a given multisampled kind to the device",
            CreationError::Size(_) => "Unsupported size in one of the dimensions",
            CreationError::Data(_) => "The given data has a different size than the target texture slice",
            CreationError::Usage(_) => "The expected texture usage mode is not supported by a graphic API",
        }
    }
}

/// Error creating an `ImageView`.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewError {
    /// The required usage flag is not present in the image.
    Usage(Usage),
    /// Selected mip levels doesn't exist.
    Level(Level),
    /// Selected array layer doesn't exist.
    Layer(LayerError),
    /// An incompatible format was requested for the view.
    BadFormat,
    /// The backend refused for some reason.
    Unsupported,
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let description = self.description();
        match *self {
            ViewError::Usage(usage) => write!(f, "{}: {:?}", description, usage),
            ViewError::Level(level) => write!(f, "{}: {}", description, level),
            ViewError::Layer(ref layer) => write!(f, "{}: {}", description, layer),
            _ => write!(f, "{}", description)
        }
    }
}

impl Error for ViewError {
    fn description(&self) -> &str {
        match *self {
            ViewError::Usage(_) =>
                "The required usage flag is not present in the image",
            ViewError::Level(_) =>
                "Selected mip level doesn't exist",
            ViewError::Layer(_) =>
                "Selected array layer doesn't exist",
            ViewError::BadFormat =>
                "An incompatible format was requested for the view",
            ViewError::Unsupported =>
                "The backend refused for some reason",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            ViewError::Layer(ref e) => Some(e),
            _ => None,
        }
    }
}


/// An error associated with selected texture layer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LayerError {
    /// The source texture kind doesn't support array slices.
    NotExpected(Kind),
    /// Selected layer is outside of the provided range.
    OutOfBounds(Range<Layer>),
}

impl fmt::Display for LayerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            LayerError::NotExpected(kind) => write!(f, "{}: {:?}", self.description(), kind),
            LayerError::OutOfBounds(ref range) => write!(f, "{}: {:?}", self.description(), range),
        }
    }
}

impl Error for LayerError {
    fn description(&self) -> &str {
        match *self {
            LayerError::NotExpected(_) => "The source texture kind doesn't support array slices",
            LayerError::OutOfBounds(_) => "Selected layers are outside of the provided range",
        }
    }
}

/// Dimension size
pub type Size = u16;
/// Number of bits per component
pub type Bits = u8;
/// Number of MSAA samples
pub type NumSamples = u8;
/// Number of EQAA fragments
pub type NumFragments = u8;

/// Dimensions: width, height, depth, and samples.
pub type Dimensions = (Size, Size, Size, AaMode);

/// Describes the configuration of samples inside each texel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AaMode {
    /// No additional sample information
    Single,
    /// MultiSampled Anti-Aliasing (MSAA)
    Multi(NumSamples),
    /// Coverage Sampling Anti-Aliasing (CSAA/EQAA)
    Coverage(NumSamples, NumFragments),
}

impl From<NumSamples> for AaMode {
    fn from(ns: NumSamples) -> AaMode {
        if ns > 1 {
            AaMode::Multi(ns)
        } else {
            AaMode::Single
        }
    }
}

impl AaMode {
    /// Return the number of actual data fragments stored per texel.
    pub fn num_fragments(&self) -> NumFragments {
        match *self {
            AaMode::Single => 1,
            AaMode::Multi(n) => n,
            AaMode::Coverage(_, nf) => nf,
        }
    }
    /// Return true if the surface has to be resolved before sampling.
    pub fn needs_resolve(&self) -> bool {
        self.num_fragments() > 1
    }
}


/// How to [filter](https://en.wikipedia.org/wiki/Texture_filtering) the
/// texture when sampling. They correspond to increasing levels of quality,
/// but also cost. They "layer" on top of each other: it is not possible to
/// have bilinear filtering without mipmapping, for example.
///
/// These names are somewhat poor, in that "bilinear" is really just doing
/// linear filtering on each axis, and it is only bilinear in the case of 2D
/// textures. Similarly for trilinear, it is really Quadralinear(?) for 3D
/// textures. Alas, these names are simple, and match certain intuitions
/// ingrained by many years of public use of inaccurate terminology.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FilterMethod {
    /// The dumbest filtering possible, nearest-neighbor interpolation.
    Scale,
    /// Add simple mipmapping.
    Mipmap,
    /// Sample multiple texels within a single mipmap level to increase
    /// quality.
    Bilinear,
    /// Sample multiple texels across two mipmap levels to increase quality.
    Trilinear,
    /// Anisotropic filtering with a given "max", must be between 1 and 16,
    /// inclusive.
    Anisotropic(u8)
}

/// The face of a cube texture to do an operation on.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum CubeFace {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

/// A constant array of cube faces in the order they map to the hardware.
pub const CUBE_FACES: [CubeFace; 6] = [
    CubeFace::PosX, CubeFace::NegX,
    CubeFace::PosY, CubeFace::NegY,
    CubeFace::PosZ, CubeFace::NegZ,
];

/// Specifies the kind of a texture storage to be allocated.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Kind {
    /// A single one-dimensional row of texels.
    D1(Size),
    /// An array of rows of texels. Equivalent to Texture2D except that texels
    /// in different rows are not sampled, so filtering will be constrained
    /// to a single row of texels at a time.
    D1Array(Size, Layer),
    /// A traditional 2D texture, with rows arranged contiguously.
    D2(Size, Size, AaMode),
    /// An array of 2D textures. Equivalent to Texture3D except that texels in
    /// a different depth level are not sampled.
    D2Array(Size, Size, Layer, AaMode),
    /// A volume texture, with each 2D layer arranged contiguously.
    D3(Size, Size, Size),
    /// A set of 6 2D textures, one for each face of a cube.
    Cube(Size),
    /// An array of Cube textures.
    CubeArray(Size, Layer),
}

impl Kind {
    /// Get texture dimensions
    pub fn dimensions(&self) -> Dimensions {
        let s0 = AaMode::Single;
        match *self {
            Kind::D1(w) => (w, 1, 1, s0),
            Kind::D1Array(w, a) => (w, 1, a as Size, s0),
            Kind::D2(w, h, s) => (w, h, 1, s),
            Kind::D2Array(w, h, a, s) => (w, h, a as Size, s),
            Kind::D3(w, h, d) => (w, h, d, s0),
            Kind::Cube(w) => (w, w, 6, s0),
            Kind::CubeArray(w, a) => (w, w, 6 * (a as Size), s0)
        }
    }
    /// Get the dimensionality of a particular mipmap level.
    pub fn level_dimensions(&self, level: Level) -> Dimensions {
        use std::cmp::{max, min};
        // must be at least 1
        let map = |val| max(min(val, 1), val >> min(level, MAX_LEVEL));
        let (w, h, da, _) = self.dimensions();
        let dm = if self.num_slices().is_some() {
            1
        } else {
            map(da)
        };
        (map(w), map(h), dm, AaMode::Single)
    }
    /// Count the number of mipmap levels.
    pub fn num_levels(&self) -> Level {
        use std::cmp::max;
        let (w, h, d, aa) = self.dimensions();
        let dominant = max(max(w, h), d);
        if aa == AaMode::Single {
            (1..).find(|level| dominant>>level <= 1).unwrap()
        }else {
            1 // anti-aliased textures can't have mipmaps
        }
    }
    /// Return the number of slices in a texture array type, 
    /// or None for non-arrays.
    pub fn num_slices(&self) -> Option<Layer> {
        match *self {
            Kind::D1(..) | Kind::D2(..) | Kind::D3(..) | Kind::Cube(..) => None,
            Kind::D1Array(_, a) => Some(a),
            Kind::D2Array(_, _, a, _) => Some(a),
            Kind::CubeArray(_, a) => Some(a),
        }
    }
    /// Return the number of layers in an array type.
    ///
    /// Each cube face counts as separate layer.
    pub fn num_layers(&self) -> Layer {
        match *self {
            Kind::D1(..) | Kind::D2(..) | Kind::D3(..) => 1,
            Kind::Cube(..) => 6,
            Kind::D1Array(_, a) => a,
            Kind::D2Array(_, _, a, _) => a,
            Kind::CubeArray(_, a) => 6*a,
        }
    }
    /// Checks whether the `Kind` is `Cube` or `CubeArray`.
    pub fn is_cube(&self) -> bool {
        match *self {
            Kind::Cube(_) | Kind::CubeArray(_, _) => true,
            _ => false,
        }
    }
}

bitflags!(
    /// TODO: Find out if TRANSIENT_ATTACHMENT + INPUT_ATTACHMENT
    /// are applicable on backends other than Vulkan. --AP
    /// Image usage flags
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Usage: u8 {
        /// The image is used as a transfer source.
        const TRANSFER_SRC = 0x1;
        /// The image is used as a transfer destination.
        const TRANSFER_DST = 0x2;
        /// The image is used as a color attachment -- that is, color input to a rendering pass.
        const COLOR_ATTACHMENT = 0x4;
        /// The image is used as a depth attachment.
        const DEPTH_STENCIL_ATTACHMENT = 0x8;
        /// The image is a [storage image](https://www.khronos.org/registry/vulkan/specs/1.0/html/vkspec.html#descriptorsets-storageimage)
        const STORAGE = 0x10;
        /// The image is a [sampled image](https://www.khronos.org/registry/vulkan/specs/1.0/html/vkspec.html#descriptorsets-sampledimage)
        const SAMPLED = 0x20;
        ///
        const TRANSIENT_ATTACHMENT = 0x40;
        ///
        const INPUT_ATTACHMENT = 0x80;

    }
);

impl Usage {
    /// Returns true if this image can be used in transfer operations.
    pub fn can_transfer(&self) -> bool {
        self.intersects(Usage::TRANSFER_SRC | Usage::TRANSFER_DST)
    }

    /// Returns true if this image can be used as a target.
    pub fn can_target(&self) -> bool {
        self.intersects(Usage::COLOR_ATTACHMENT | Usage::DEPTH_STENCIL_ATTACHMENT)
    }
}

/// Specifies how texture coordinates outside the range `[0, 1]` are handled.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WrapMode {
    /// Tile the texture, that is, sample the coordinate modulo `1.0`, so 
    /// addressing the texture beyond an edge will "wrap" back from the
    /// other edge.
    Tile,
    /// Mirror the texture. Like tile, but uses abs(coord) before the modulo.
    Mirror,
    /// Clamp the texture to the value at `0.0` or `1.0` respectively.
    Clamp,
    /// Use border color.
    Border,
}

/// A wrapper for the LOD level of a texture.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Lod(i16);

impl From<f32> for Lod {
    fn from(v: f32) -> Lod {
        Lod((v * 8.0) as i16)
    }
}

impl Into<f32> for Lod {
    fn into(self) -> f32 {
        self.0 as f32 / 8.0
    }
}

/// A wrapper for an RGBA color with 8 bits per texel, encoded as a u32.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PackedColor(pub u32);

impl From<[f32; 4]> for PackedColor {
    fn from(c: [f32; 4]) -> PackedColor {
        PackedColor(c.iter().rev().fold(0, |u, &c| {
            (u<<8) + (c * 255.0) as u32
        }))
    }
}

impl Into<[f32; 4]> for PackedColor {
    fn into(self) -> [f32; 4] {
        let mut out = [0.0; 4];
        for i in 0 .. 4 {
            let byte = (self.0 >> (i<<3)) & 0xFF;
            out[i] = byte as f32 / 255.0;
        }
        out
    }
}

/// Specifies how to sample from a texture.
// TODO: document the details of sampling.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SamplerInfo {
    /// Filter method to use.
    pub filter: FilterMethod,
    /// Wrapping mode for each of the U, V, and W axis (S, T, and R in OpenGL
    /// speak).
    pub wrap_mode: (WrapMode, WrapMode, WrapMode),
    /// This bias is added to every computed mipmap level (N + lod_bias). For
    /// example, if it would select mipmap level 2 and lod_bias is 1, it will
    /// use mipmap level 3.
    pub lod_bias: Lod,
    /// This range is used to clamp LOD level used for sampling.
    pub lod_range: Range<Lod>,
    /// Comparison mode, used primary for a shadow map.
    pub comparison: Option<Comparison>,
    /// Border color is used when one of the wrap modes is set to border.
    pub border: PackedColor,
}

impl SamplerInfo {
    /// Create a new sampler description with a given filter method and wrapping mode, using no LOD
    /// modifications.
    pub fn new(filter: FilterMethod, wrap: WrapMode) -> SamplerInfo {
        SamplerInfo {
            filter: filter,
            wrap_mode: (wrap, wrap, wrap),
            lod_bias: Lod(0),
            lod_range: Lod(-8000)..Lod(8000),
            comparison: None,
            border: PackedColor(0),
        }
    }
}

/// Texture resource view descriptor.
/// Legacy code to be removed, per msiglreith.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(missing_docs)]
pub struct ResourceDesc {
    pub channel: format::ChannelType,
    pub layer: Option<Layer>,
    pub levels: Range<Level>,
    pub swizzle: format::Swizzle,
}

/// Texture render view descriptor.
/// Legacy code to be removed, per msiglreith.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(missing_docs)]
pub struct RenderDesc {
    pub channel: format::ChannelType,
    pub level: Level,
    pub layer: Option<Layer>,
}

bitflags!(
    /// Depth-stencil read-only flags
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct DepthStencilFlags: u8 {
        /// Depth is read-only in the view.
        const RO_DEPTH    = 0x1;
        /// Stencil is read-only in the view.
        const RO_STENCIL  = 0x2;
        /// Both depth and stencil are read-only.
        const RO_DEPTH_STENCIL = 0x3;
    }
);

/// Texture depth-stencil view descriptor.
/// Legacy code to be removed, per msiglreith.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DepthStencilDesc {
    pub level: Level,
    pub layer: Option<Layer>,
    pub flags: DepthStencilFlags,
}

impl From<RenderDesc> for DepthStencilDesc {
    fn from(rd: RenderDesc) -> DepthStencilDesc {
        DepthStencilDesc {
            level: rd.level,
            layer: rd.layer,
            flags: DepthStencilFlags::empty(),
        }
    }
}

/// Specifies options for how memory for an image is arranged.
/// These are hints to the GPU driver and may or may not have actual
/// performance effects, but describe constraints on how the data
/// may be used that a program *must* obey. They do not specify
/// how channel values or such are laid out in memory; the actual
/// image data is considered opaque.
///
/// Details may be found in [the Vulkan spec](https://www.khronos.org/registry/vulkan/specs/1.0/html/vkspec.html#resources-image-layouts)
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ImageLayout {
    /// General purpose, no restrictions on usage.
    General,
    /// Must only be used as a color attachment in a framebuffer.
    ColorAttachmentOptimal,
    /// Must only be used as a depth attachment in a framebuffer.
    DepthStencilAttachmentOptimal,
    /// Must only be used as a depth attachment in a framebuffer,
    /// or as a read-only depth or stencil buffer in a shader.
    DepthStencilReadOnlyOptimal,
    /// Must only be used as a read-only image in a shader.
    ShaderReadOnlyOptimal,
    /// Must only be used as the source for a transfer command.
    TransferSrcOptimal,
    /// Must only be used as the destination for a transfer command.
    TransferDstOptimal,
    /// No layout, does not support device access.  Only valid as a
    /// source layout when transforming data to a specific destination
    /// layout or initializing data.  Does NOT guarentee that the contents 
    /// of the source buffer are preserved.
    Undefined, //TODO: consider Option<> instead?
    /// Like `Undefined`, but does guarentee that the contents of the source
    /// buffer are preserved.
    Preinitialized,
    /// The layout that an image must be in to be presented to the display.
    Present,
}

bitflags!(
    /// Bitflags to describe how memory in an image or buffer can be accessed.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Access: u16 {
        /// Read state but can only be combined with `COLOR_ATTACHMENT_WRITE`.
        const COLOR_ATTACHMENT_READ = 0x1;
        /// Write-only state but can be combined with `COLOR_ATTACHMENT_READ`.
        const COLOR_ATTACHMENT_WRITE = 0x2;
        /// Read access to the buffer in a copy operation.
        const TRANSFER_READ = 0x4;
        /// Write access to the buffer in a copy operation.
        const TRANSFER_WRITE = 0x8;
        /// Read-only state for SRV access, or combine with `SHADER_WRITE` to have r/w access to UAV.
        const SHADER_READ = 0x10;
        /// Writeable state for UAV access.
        /// Combine with `SHADER_READ` to have r/w access to UAV.
        const SHADER_WRITE = 0x20;
        /// Read access to a depth/stencil attachment in a depth or stencil operation.
        const DEPTH_STENCIL_ATTACHMENT_READ = 0x40;
        /// Write access to a depth/stencil attachment in a depth or stencil operation.
        const DEPTH_STENCIL_ATTACHMENT_WRITE = 0x80;
        /// Read access for raw memory to be accessed by the host system (ie, CPU).
        const HOST_READ = 0x100;
        /// Write access for raw memory to be accessed by the host system.
        const HOST_WRITE = 0x200;
        /// Read access for memory to be accessed by a non-specific entity.  This may
        /// be the host system, or it may be something undefined or specified by an
        /// extension.
        const MEMORY_READ = 0x400;
        /// Write access for memory to be accessed by a non-specific entity.
        const MEMORY_WRITE = 0x800;
        /// Read access to an input attachment from within a fragment shader.
        const INPUT_ATTACHMENT_READ = 0x1000;
    }
);

/// Image state, combining access methods and the image's layout.
pub type State = (Access, ImageLayout);

/// Selector of a concrete subresource in an image.
pub type Subresource = (Level, Layer);

/// A subset of resource layers contained within an image's level.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SubresourceLayers {
    /// Included aspects: color/depth/stencil
    pub aspects: format::Aspects,
    /// Selected mipmap level
    pub level: Level,
    /// Included array levels
    pub layers: Range<Layer>,
}

/// A subset of resources contained within an image.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SubresourceRange {
    /// Included aspects: color/depth/stencil
    pub aspects: format::Aspects,
    /// Included mipmap levels
    pub levels: Range<Level>,
    /// Included array levels
    pub layers: Range<Layer>,
}
