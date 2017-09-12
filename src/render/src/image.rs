use core;
use memory::Memory;

pub use core::image::{
    CreationError, Kind, AaMode, Size, Level, Layer, Dimensions,
    SamplerInfo, SubresourceRange
};
pub use core::image::{Usage,
    TRANSFER_SRC, TRANSFER_DST,
    COLOR_ATTACHMENT, DEPTH_STENCIL_ATTACHMENT,
    SAMPLED
};

/// Texture storage descriptor.
#[allow(missing_docs)]
#[derive(Debug)]
pub struct Info {
    pub usage: Usage,
    pub kind: Kind,
    pub mip_levels: Level,
    pub format: core::format::Format,
    pub memory: Memory,
}
/*
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

    /// Get the raw image info for a given mip.
    pub fn to_raw_image_info(&self, mip: Level) -> RawImageInfo {
        self.to_image_info(mip).convert(self.format)
    }
}
*/
