use hal;
use memory::Memory;

pub use hal::image::{
    CreationError, Kind, AaMode, Size, Level, Layer, Dimensions,
    AspectFlags, SamplerInfo, ViewError, Usage,
    Subresource, SubresourceLayers, SubresourceRange,
};
pub use hal::image::{
    TRANSFER_SRC, TRANSFER_DST,
    COLOR_ATTACHMENT, DEPTH_STENCIL_ATTACHMENT,
    SAMPLED,
};

#[allow(missing_docs)]
#[derive(Debug)]
pub struct Info {
    pub aspects: AspectFlags,
    pub usage: Usage,
    pub kind: Kind,
    pub mip_levels: Level,
    pub format: hal::format::Format,
    pub origin: Origin,
    pub(crate) stable_state: hal::image::State,
}

#[derive(Debug)]
pub enum Origin {
    Backbuffer,
    User(Memory),
}
