//! Displays.
//!
//! A display represent a physical display collected from an Adapter

use crate::{
    window::{Extent2D, Offset2D},
    Backend,
};
pub mod display_control;
pub use display_control::*;

bitflags! {
    /**
    List of the hardware display transformations
    */
    pub struct SurfaceTransformFlags : u32 {
        /// Specify that image content is presented without being transformed.
        const IDENTITY = 1;
        /// Specify that image content is rotated 90 degrees clockwise.
        const ROTATE_90 = 2;
        /// Specify that image content is rotated 180 degrees clockwise.
        const ROTATE_180 = 4;
        /// Specify that image content is rotated 270 degrees clockwise.
        const ROTATE_270 = 8;
        /// Specify that image content is mirrored horizontally.
        const HORIZONTAL_MIRROR = 16;
        /// Specify that image content is mirrored horizontally, then rotated 90 degrees clockwise.
        const HORIZONTAL_MIRROR_ROTATE_90 = 32;
        /// Specify that image content is mirrored horizontally, then rotated 180 degrees clockwise.
        const HORIZONTAL_MIRROR_ROTATE_180 = 64;
        /// Specify that image content is mirrored horizontally, then rotated 270 degrees clockwise.
        const HORIZONTAL_MIRROR_ROTATE_270 = 128;
        /// Specify that the presentation transform is not specified, and is instead determined by platform-specific considerations and mechanisms outside Vulkan.
        const INHERIT = 256;
    }
}
impl From<SurfaceTransform> for SurfaceTransformFlags {
    fn from(surface_transformation: SurfaceTransform) -> Self {
        match surface_transformation {
            SurfaceTransform::IDENTITY => Self::IDENTITY,
            SurfaceTransform::ROTATE_90 => Self::ROTATE_90,
            SurfaceTransform::ROTATE_180 => Self::ROTATE_180,
            SurfaceTransform::ROTATE_270 => Self::ROTATE_270,
            SurfaceTransform::HORIZONTAL_MIRROR => Self::HORIZONTAL_MIRROR,
            SurfaceTransform::HORIZONTAL_MIRROR_ROTATE_90 => Self::HORIZONTAL_MIRROR_ROTATE_90,
            SurfaceTransform::HORIZONTAL_MIRROR_ROTATE_180 => Self::HORIZONTAL_MIRROR_ROTATE_180,
            SurfaceTransform::HORIZONTAL_MIRROR_ROTATE_270 => Self::HORIZONTAL_MIRROR_ROTATE_270,
            SurfaceTransform::INHERIT => Self::INHERIT,
        }
    }
}

#[derive(Debug, PartialEq)]
#[allow(non_camel_case_types)]
/**
List of the hardware display transformations
*/
pub enum SurfaceTransform {
    /// Specify that image content is presented without being transformed.
    IDENTITY,
    /// Specify that image content is rotated 90 degrees clockwise.
    ROTATE_90,
    /// Specify that image content is rotated 180 degrees clockwise.
    ROTATE_180,
    /// Specify that image content is rotated 270 degrees clockwise.
    ROTATE_270,
    /// Specify that image content is mirrored horizontally.
    HORIZONTAL_MIRROR,
    /// Specify that image content is mirrored horizontally, then rotated 90 degrees clockwise.
    HORIZONTAL_MIRROR_ROTATE_90,
    /// Specify that image content is mirrored horizontally, then rotated 180 degrees clockwise.
    HORIZONTAL_MIRROR_ROTATE_180,
    /// Specify that image content is mirrored horizontally, then rotated 270 degrees clockwise.
    HORIZONTAL_MIRROR_ROTATE_270,
    /// Specify that the presentation transform is not specified, and is instead determined by platform-specific considerations and mechanisms outside Vulkan.
    INHERIT,
}
impl Default for SurfaceTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/**
General information about the a [display][Display].
*/
#[derive(Debug)]
pub struct DisplayInfo {
    /// Name of the display. Generally, this will be the name provided by the display’s EDID.
    pub name: Option<String>,
    /// Physical width and height of the visible portion of the display, in millimeters.
    pub physical_dimensions: Extent2D,
    /// Physical, native, or preferred resolution of the display.
    pub physical_resolution: Extent2D,
    /// Description of the supported transforms by the display.
    pub supported_transforms: SurfaceTransformFlags,
    /// Tells whether the planes on the display can have their z order changed. If true, the application can re-arrange the planes on this display in any order relative to each other.
    pub plane_reorder_possible: bool,
    /// Tells whether the display supports self-refresh/internal buffering. If true, the application can submit persistent present operations on swapchains created against this display.
    pub persistent_content: bool,
}

bitflags! {
    /**
    Alpha mode used in display surface creation
    */
    pub struct DisplayPlaneAlphaFlags : u32 {
        /// Specifies that the source image will be treated as opaque
        const OPAQUE = 1;
        /// Specifies that the provided global alpha value will be applied to all pixels in the source image.
        const GLOBAL = 2;
        /// Specifies that the alpha value will be determined by the alpha channel of the source image’s pixels.
        /// If the source format contains no alpha values, no blending will be applied. The source alpha values are not premultiplied into the source image’s other color channels.
        const PER_PIXEL = 4;
        /// Equivalent to PerPixel, except the source alpha values are assumed to be premultiplied into the source image’s other color channels.
        const PER_PIXEL_PREMULTIPLIED = 8;
    }
}
impl From<DisplayPlaneAlpha> for DisplayPlaneAlphaFlags {
    fn from(display_plane_alpha: DisplayPlaneAlpha) -> Self {
        match display_plane_alpha {
            DisplayPlaneAlpha::OPAQUE => Self::OPAQUE,
            DisplayPlaneAlpha::GLOBAL(_) => Self::GLOBAL,
            DisplayPlaneAlpha::PER_PIXEL => Self::PER_PIXEL,
            DisplayPlaneAlpha::PER_PIXEL_PREMULTIPLIED => Self::PER_PIXEL_PREMULTIPLIED,
        }
    }
}

/**
Alpha mode used in display surface creation
*/
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum DisplayPlaneAlpha {
    /// Specifies that the source image will be treated as opaque
    OPAQUE,
    /// Specifies that the provided global alpha value will be applied to all pixels in the source image.
    GLOBAL(f32),
    /// Specifies that the alpha value will be determined by the alpha channel of the source image’s pixels. If the source format contains no alpha values, no blending will be applied. The source alpha values are not premultiplied into the source image’s other color channels.
    PER_PIXEL,
    /// Equivalent to PerPixel, except the source alpha values are assumed to be premultiplied into the source image’s other color channels.
    PER_PIXEL_PREMULTIPLIED,
}

impl Default for DisplayPlaneAlpha {
    fn default() -> Self {
        Self::OPAQUE
    }
}

// This implementation is done to ignore differences on the value in DisplayPlaneAlpha::Global
impl PartialEq for DisplayPlaneAlpha {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DisplayPlaneAlpha::OPAQUE, DisplayPlaneAlpha::OPAQUE) => true,
            (DisplayPlaneAlpha::GLOBAL(_), DisplayPlaneAlpha::GLOBAL(_)) => true,
            (DisplayPlaneAlpha::PER_PIXEL, DisplayPlaneAlpha::PER_PIXEL) => true,
            (
                DisplayPlaneAlpha::PER_PIXEL_PREMULTIPLIED,
                DisplayPlaneAlpha::PER_PIXEL_PREMULTIPLIED,
            ) => true,
            _ => false,
        }
    }
}

/// Error occurring while creating a display plane.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum DisplayModeError {
    /// Display error.
    #[error(transparent)]
    OutOfMemory(#[from] crate::device::OutOfMemory),
    /// Unsupported resolution and refresh rate combination
    #[error("Unsupported resolution and refresh rate combination")]
    UnsupportedDisplayMode,
}

/// Error occurring while creating a display plane surface.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum DisplayPlaneSurfaceError {
    /// Display error.
    #[error(transparent)]
    OutOfMemory(#[from] crate::device::OutOfMemory),
    /// Unsupported resolution and refresh rate combination
    #[error("Unsupported parameters used")]
    UnsupportedParameters,
}

/**
Representation of a display
*/
#[derive(Debug)]
pub struct Display<B: Backend> {
    /// The display handle.
    pub handle: B::Display,
    /// General informations about this display.
    pub info: DisplayInfo,
    /// Builtin display modes
    pub modes: Vec<DisplayMode<B>>,
}

/**
General information about the a [DisplayMode][DisplayMode].
*/
#[derive(Debug)]
pub struct DisplayMode<B: Backend> {
    /// The display mode handle
    pub handle: B::DisplayMode,
    /// Resolution
    pub resolution: (u32, u32),
    /// Refresh rate
    pub refresh_rate: u32,
}

/**
Representation of a plane
*/
#[derive(Debug)]
pub struct Plane {
    /// The plane handle.
    pub handle: u32,
    /// The current index on the z stack.
    pub z_index: u32,
}

/**
Represent a combination of [display mode][DisplayMode] (so [display][Display] and resolution) and a plane
*/
#[derive(Debug)]
pub struct DisplayPlane<'a, B: Backend> {
    /// Display mode
    pub display_mode: &'a DisplayMode<B>,
    /// Plane index
    pub plane: &'a Plane,
    /// Supported alpha capabilities
    pub supported_alpha: Vec<DisplayPlaneAlpha>,
    /// The minimum source rectangle offset supported by this plane using the specified mode.
    pub min_src_position: Offset2D,
    /// The maximum source rectangle offset supported by this plane using the specified mode. The x and y components of max_src_position must each be greater than or equal to the x and y components of min_src_position, respectively.
    pub max_src_position: Offset2D,
    /// The minimum source rectangle size supported by this plane using the specified mode.
    pub min_src_extent: Extent2D,
    /// The maximum source rectangle size supported by this plane using the specified mode.
    pub max_src_extent: Extent2D,
    /// Same as min_src_position. but applied to destination.
    pub min_dst_position: Offset2D,
    /// Same as max_src_position. but applied to destination.
    pub max_dst_position: Offset2D,
    /// Same as min_src_extent. but applied to destination.
    pub min_dst_extent: Extent2D,
    /// Same as max_src_extent. but applied to destination.
    pub max_dst_extent: Extent2D,
}
