//! RenderPass handling.

use format::Format;
use image;
use pso::PipelineStage;
use Backend;
use std::ops::Range;

/// Specifies the operation which will be applied at the beginning of a subpass.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AttachmentOps {
    /// Whether or not data from the load operation will be preserved after the subpass.
    pub load: AttachmentLoadOp,
    /// Whether or not data from the store operation will be preserved after the subpass.
    pub store: AttachmentStoreOp,
}

impl AttachmentOps {
    /// Specifies `DontCare` for both load and store op.
    pub const DONT_CARE: Self = AttachmentOps {
        load: AttachmentLoadOp::DontCare,
        store: AttachmentStoreOp::DontCare,
    };
    /// Convenience function to create a new `AttachmentOps`.
    pub fn new(load: AttachmentLoadOp, store: AttachmentStoreOp) -> Self {
        AttachmentOps {
            load,
            store,
        }
    }

    /// A method to provide `AttachmentOps::DONT_CARE` to things that expect
    /// a default function rather than a value.
    #[cfg(feature = "serde")]
    fn whatever() -> Self {
        Self::DONT_CARE
    }
}

/// An `Attachment` is a description of a resource provided to a render subpass.
/// It includes things such as render targets, images that were produced from
/// previous subpasses, etc.
#[derive(Clone, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Attachment {
    /// Attachment format
    ///
    /// In the most cases `format` is not `None`. It should be only used for
    /// creating dummy renderpasses, which are used as placeholder for compatible
    /// renderpasses.
    pub format: Option<Format>,
    /// Load and store operations of the attachment
    pub ops: AttachmentOps,
    /// Load and store operations of the stencil aspect, if any
    #[cfg_attr(feature = "serde", serde(default = "AttachmentOps::whatever"))]
    pub stencil_ops: AttachmentOps,
    /// Initial and final image layouts of the renderpass.
    pub layouts: Range<AttachmentLayout>,
}

/// Index of an attachment within a framebuffer/renderpass,
pub type AttachmentId = usize;
/// Reference to an attachment by index and expected image layout.
pub type AttachmentRef = (AttachmentId, AttachmentLayout);

/// What other subpasses a particular subpass depends on.
#[derive(Copy, Clone, Debug, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SubpassRef {
    /// The subpass depends on something that was submitted to the
    /// queue before the render pass began.
    External,
    /// The subpass depends on another subpass with the given index.
    Pass(usize),
}

/// Expresses a dependency between multiple subpasses.  This is used
/// both to describe a source or destination subpass; data either 
/// explicitly passes from this subpass to the next or from another 
/// subpass into this one.
#[derive(Clone, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SubpassDependency {
    /// Other subpasses this one depends on.
    pub passes: Range<SubpassRef>,
    /// Other pipeline stages this subpass depends on.
    pub stages: Range<PipelineStage>,
    /// Resource accesses this subpass depends on.
    pub accesses: Range<image::Access>,
}

/// Description of a subpass for renderpass creation.
pub struct SubpassDesc<'a> {
    /// Which attachments will be used as color buffers.
    pub colors: &'a [AttachmentRef],
    /// Which attachments will be used as depth/stencil buffers.
    pub depth_stencil: Option<&'a AttachmentRef>,
    /// Which attachments MAY be used by this subpass.
    pub inputs: &'a [AttachmentRef],
    /// Attachments that are not used by the subpass but must be preserved to be
    /// passed on to subsequent passes.
    pub preserves: &'a [AttachmentId],
}

/// Index of a subpass.
pub type SubpassId = usize;

/// A sub-pass borrow of a pass.
#[derive(Debug)]
pub struct Subpass<'a, B: Backend> {
    /// Index of the subpass
    pub index: SubpassId,
    /// Main pass borrow.
    pub main_pass: &'a B::RenderPass,
}

impl<'a, B: Backend> Clone for Subpass<'a, B> {
    fn clone(&self) -> Self {
        Subpass {
            index: self.index,
            main_pass: self.main_pass,
        }
    }
}

impl<'a, B: Backend> PartialEq for Subpass<'a, B> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index &&
        self.main_pass as *const _ == other.main_pass as *const _
    }
}

impl<'a, B: Backend> Copy for Subpass<'a, B> {}
impl<'a, B: Backend> Eq for Subpass<'a, B> {}
