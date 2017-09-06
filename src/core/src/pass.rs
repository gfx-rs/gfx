//! RenderPass handling.

use format::Format;
use image;
use pso::PipelineStage;
use Backend;
use std::ops::Range;

/// Specifies the operation which will be applied at the beginning of a subpass.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum AttachmentLoadOp {
    /// Preserve existing content in the attachment.
    Load,
    /// Clear the attachment.
    Clear,
    /// Attachment content will be undefined.
    DontCare,
}

///
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum AttachmentStoreOp {
    /// Content written to the attachment will be preserved.
    Store,
    /// Attachment content will be undefined.
    DontCare,
}

/// Image layout of an attachment.
pub type AttachmentLayout = image::ImageLayout;

/// Attachment operations.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct AttachmentOps {
    ///
    pub load: AttachmentLoadOp,
    ///
    pub store: AttachmentStoreOp,
}

impl AttachmentOps {
    ///
    pub const DONT_CARE: Self = AttachmentOps {
        load: AttachmentLoadOp::DontCare,
        store: AttachmentStoreOp::DontCare,
    };
    ///
    pub fn new(load: AttachmentLoadOp, store: AttachmentStoreOp) -> Self {
        AttachmentOps {
            load,
            store,
        }
    }
}

///
#[derive(Clone, Debug, Hash)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Attachment {
    ///
    pub format: Format,
    /// load and store operations of the attachment
    pub ops: AttachmentOps,
    /// load and store operations of the stencil aspect, if any
    pub stencil_ops: AttachmentOps,
    /// Initial and final image layouts of the renderpass.
    pub layouts: Range<AttachmentLayout>,
}

/// Reference to an attachment by index and expected image layout.
pub type AttachmentRef = (usize, AttachmentLayout);

///
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum SubpassRef {
    ///
    External,
    ///
    Pass(usize),
}

/// Specifies dependencies between subpasses.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct SubpassDependency {
    ///
    pub passes: Range<SubpassRef>,
    ///
    pub stages: Range<PipelineStage>,
    ///
    pub accesses: Range<image::Access>,
}

/// Description of a subpass for renderpass creation.
pub struct SubpassDesc<'a> {
    ///
    pub color_attachments: &'a [AttachmentRef],
}

/// A sub-pass borrow of a pass.
pub struct SubPass<'a, B: Backend> {
    /// Index of the sub pass
    pub index: usize,
    /// Main pass borrow.
    pub main_pass: &'a B::RenderPass,
}
