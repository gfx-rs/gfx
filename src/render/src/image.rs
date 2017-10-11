use core;
use memory::Memory;

pub use core::image::{
    CreationError, Kind, AaMode, Size, Level, Layer, Dimensions,
    AspectFlags, SamplerInfo, ViewError,
    Subresource, SubresourceLayers, SubresourceRange,
};
pub use core::image::{Usage,
    TRANSFER_SRC, TRANSFER_DST,
    COLOR_ATTACHMENT, DEPTH_STENCIL_ATTACHMENT,
    SAMPLED
};

#[allow(missing_docs)]
#[derive(Debug)]
pub struct Info {
    pub aspects: AspectFlags,
    pub usage: Usage,
    pub kind: Kind,
    pub mip_levels: Level,
    pub format: core::format::Format,
    pub origin: Origin,
    pub(crate) stable_state: core::image::State,
}

#[derive(Debug)]
pub enum Origin {
    Backbuffer,
    User(Memory),
}
