use core;
use memory;

pub use core::image::{
    CreationError, Kind, AaMode, Size, Level, Layer, Usage, Dimensions,
    ImageInfoCommon, RawImageInfo, NewImageInfo,
    SamplerInfo, SubresourceRange
};

/// Texture storage descriptor.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Info {
    pub kind: Kind,
    pub levels: Level,
    pub format: core::format::SurfaceType,
    pub bind: memory::Bind,
    pub usage: memory::Usage,
    // TODO: do we need things from image::Usage ?
}

impl Info {
    /// Get image info for a given mip.
    pub fn to_image_info(&self, mip: Level) -> NewImageInfo {
        let (w, h, d, _) = self.kind.get_level_dimensions(mip);
        ImageInfoCommon {
            xoffset: 0,
            yoffset: 0,
            zoffset: 0,
            width: w,
            height: h,
            depth: d,
            format: (),
            mipmap: mip,
        }
    }

    /// Get the raw image info for a given mip and a channel type.
    pub fn to_raw_image_info(&self, cty: core::format::ChannelType, mip: Level) -> RawImageInfo {
        let format = core::format::Format(self.format, cty.into());
        self.to_image_info(mip).convert(format)
    }
}
