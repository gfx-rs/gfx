//! Displays.
//!
//! A display represent a physical display collected from an Adapter

use crate::{Backend,window::{Offset2D,Extent2D}};

/**
List of the hardware display transformations
*/
#[derive(Debug,PartialEq)]
pub enum SurfaceTransformation
{
    /// Specify that image content is presented without being transformed
    Identity,
    /// Specify that image content is rotated 90 degrees clockwise
    Rotate90,
    /// Specify that image content is rotated 180 degrees clockwise
    Rotate180,
    /// Specify that image content is rotated 270 degrees clockwise.
    Rotate270,
    /// Specify that image content is mirrored horizontally.
    HorizontalMirror,
    /// Specify that image content is mirrored horizontally, then rotated 90 degrees clockwise.
    HorizontalMirrorRotate90,
    /// Specify that image content is mirrored horizontally, then rotated 180 degrees clockwise.
    HorizontalMirrorRotate180,
    /// Specify that image content is mirrored horizontally, then rotated 270 degrees clockwise.
    HorizontalMirrorRotate270,
    /// Specify that the presentation transform is not specified, and is instead determined by platform-specific considerations and mechanisms outside Vulkan.
    Inherit
}
impl Default for SurfaceTransformation
{
    fn default() -> Self { Self::Identity }
}

/**
General information about the a [display][Display].
*/
#[derive(Debug)]
pub struct DisplayInfo
{
    /// Name of the display. Generally, this will be the name provided by the display’s EDID.
    pub name: Option<String>,
    /// Physical width and height of the visible portion of the display, in millimeters.
    pub physical_dimensions: Extent2D,
    /// Physical, native, or preferred resolution of the display.
    pub physical_resolution: Extent2D,
    /// Description of the supported transforms by the display.
    pub supported_transforms: Vec<SurfaceTransformation>,
    /// Tells whether the planes on the display can have their z order changed. If true, the application can re-arrange the planes on this display in any order relative to each other.
    pub plane_reorder_possible: bool,
    /// Tells whether the display supports self-refresh/internal buffering. If true, the application can submit persistent present operations on swapchains created against this display.
    pub persistent_content: bool
}

/**
Alpha mode used in display surface creation
*/
#[derive(Debug)]
pub enum DisplayPlaneAlpha
{
    /// Specifies that the source image will be treated as opaque
    Opaque,
    /// Specifies that the provided global alpha value will be applied to all pixels in the source image.
    Global(f32),
    /// Specifies that the alpha value will be determined by the alpha channel of the source image’s pixels. If the source format contains no alpha values, no blending will be applied. The source alpha values are not premultiplied into the source image’s other color channels.
    PerPixel,
    /// Equivalent to PerPixel, except the source alpha values are assumed to be premultiplied into the source image’s other color channels.
    PerPixelPremultiplied
}

impl Default for DisplayPlaneAlpha
{
    fn default() -> Self { Self::Opaque }
}

// This implementation is done to ignore differences on the value in DisplayPlaneAlpha::Global
impl PartialEq for DisplayPlaneAlpha {
    fn eq(&self, other: &Self) -> bool {
        match (self,other)
        {
            (DisplayPlaneAlpha::Opaque,DisplayPlaneAlpha::Opaque)=>true,
            (DisplayPlaneAlpha::Global(_),DisplayPlaneAlpha::Global(_))=>true,
            (DisplayPlaneAlpha::PerPixel,DisplayPlaneAlpha::PerPixel)=>true,
            (DisplayPlaneAlpha::PerPixelPremultiplied,DisplayPlaneAlpha::PerPixelPremultiplied)=>true,
            _=>false
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
pub struct Display<'a,B: Backend>
{
    /// The physical device.
    pub physical_device: &'a B::PhysicalDevice,
    /// The display handle.
    pub handle: B::Display,
    /// General information about this display.
    pub info: DisplayInfo
}

/**
General information about the a [DisplayMode][DisplayMode].
*/
#[derive(Debug)]
pub struct DisplayMode<'a,B: Backend>
{
    /// The display
    pub display: &'a Display<'a,B>,
    /// The display mode handle
    pub handle: B::DisplayMode,
    /// Resolution
    pub resolution: (u32,u32),
    /// Refresh rate
    pub refresh_rate: u32
}

/**
Representation of a plane
*/
#[derive(Debug)]
pub struct Plane<'a,B: Backend>
{
    /// The physical device.
    pub physical_device: &'a B::PhysicalDevice,
    /// The plane handle.
    pub handle: B::Plane,
    /// The current index on the z stack.
    pub z_index: u32
}


/**
Represent a combination of [display mode][DisplayMode] (so [display][Display] and resolution) and a plane
*/
#[derive(Debug)]
pub struct DisplayPlane<'a,B: Backend>
{
    /// Display mode
    pub display_mode: &'a DisplayMode<'a,B>,
    /// Plane index
    pub plane: &'a Plane<'a,B>,
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
    pub max_dst_extent: Extent2D
}

