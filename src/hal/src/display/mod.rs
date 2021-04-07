//! Displays.
//!
//! A display represent a physical display collected from an Adapter

use crate::Backend;

/**
List of the supported hardware display transformations
*/
#[derive(Debug,Default)]
pub struct SurfaceTransformations
{
    /// Specify that image content is presented without being transformed
    pub identity: bool,
    /// Specify that image content is rotated 90 degrees clockwise
    pub rotate_90: bool,
    /// Specify that image content is rotated 180 degrees clockwise
    pub rotate_180: bool,
    /// Specify that image content is rotated 270 degrees clockwise.
    pub rotate_270: bool,
    /// Specify that image content is mirrored horizontally.
    pub horizontal_mirror: bool,
    /// Specify that image content is mirrored horizontally, then rotated 90 degrees clockwise.
    pub horizontal_mirror_rotate_90: bool,
    /// Specify that image content is mirrored horizontally, then rotated 180 degrees clockwise.
    pub horizontal_mirror_rotate_180: bool,
    /// Specify that image content is mirrored horizontally, then rotated 270 degrees clockwise.
    pub horizontal_mirror_rotate_270: bool,
    /// Specify that the presentation transform is not specified, and is instead determined by platform-specific considerations and mechanisms outside Vulkan.
    pub inherit: bool
}

/**
List of the supported hardware display transformations
*/
#[derive(Debug)]
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
    pub name: String,
    /// Physical width and height of the visible portion of the display, in millimeters.
    pub physical_dimensions: (u32,u32),
    /// Physical, native, or preferred resolution of the display.
    pub physical_resolution: (u32,u32),
    /// Description of the supported transforms by the display.
    pub supported_transforms: Vec<SurfaceTransformation>,
    /// Tells whether the planes on the display can have their z order changed. If true, the application can re-arrange the planes on this display in any order relative to each other.
    pub plane_reorder_possible: bool,
    /// Tells whether the display supports self-refresh/internal buffering. If true, the application can submit persistent present operations on swapchains created against this display.
    pub persistent_content: bool
}

/**
General information about the a [DisplayMode][DisplayMode].
*/
#[derive(Debug)]
pub struct DisplayMode<B: Backend>
{
    /// Actual [display mode][DisplayMode].
    pub handle: B::DisplayMode,
    /// Resolution
    pub resolution: (u32,u32),
    /// Refresh rate
    pub refresh_rate: u32
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


/**
Representation of a display
*/
#[derive(Debug)]
pub struct Display<'a,B: Backend>
{
    /// General information about this display.
    pub info: DisplayInfo,
    /// Actual [physical device][PhysicalDevice].
    pub physical_device: &'a B::PhysicalDevice,
    /// Actual [display][Display].
    pub handle: B::Display,
    /// Actual [modes][DisplayMode].
    pub modes: Vec<DisplayMode<B>>,
    /// Actual planes count.
    pub planes_count: u32
}



