use format::Format;
use memory::{ImageAccess, ImageLayout};
use pso::PipelineStage;

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum AttachmentLoadOp {
    Load,
    Clear,
    DontCare,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum AttachmentStoreOp {
    Store,
    DontCare,
}

pub type AttachmentLayout = ImageLayout;

#[derive(Clone, Debug, Hash)]
pub struct Attachment {
    pub format: Format,
    pub load_op: AttachmentLoadOp,
    pub store_op: AttachmentStoreOp,
    pub stencil_load_op: AttachmentLoadOp,
    pub stencil_store_op:AttachmentStoreOp,
    pub src_layout: AttachmentLayout,
    pub dst_layout: AttachmentLayout,
}

pub type AttachmentRef = (usize, AttachmentLayout);

#[derive(Copy, Clone, Debug)]
pub enum SubpassRef {
    External,
    Pass(usize),
}

pub struct SubpassDependency {
    pub src_pass: SubpassRef,
    pub dst_pass: SubpassRef,
    pub src_stage: PipelineStage,
    pub dst_stage: PipelineStage,
    pub src_access: ImageAccess,
    pub dst_access: ImageAccess,
}

// TODO
pub struct SubpassDesc<'a> {
    pub color_attachments: &'a [AttachmentRef],
}
