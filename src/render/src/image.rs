use hal;
use memory::Memory;

pub use hal::format::Aspects;
pub use hal::image::{
    CreationError, Kind, ViewKind, Extent, Size, Level, Layer,
    SamplerInfo, ViewError, Usage, StorageFlags,
    Subresource, SubresourceLayers, SubresourceRange,
};

#[allow(missing_docs)]
#[derive(Debug)]
pub struct Info {
    pub aspects: Aspects,
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
