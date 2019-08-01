//! RenderPass handling.

use crate::{format::Format, image, pso::PipelineStage, Backend};

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
pub type AttachmentLayout = image::Layout;

/// Attachment operations.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AttachmentOps {
    /// Indicates how the data of the attachment will be loaded at first usage at
    /// the beginning of the subpass.
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

    /// Specifies `Load` for load op and `Store` for store op.
    pub const PRESERVE: Self = AttachmentOps {
        load: AttachmentLoadOp::Load,
        store: AttachmentStoreOp::Store,
    };

    /// Convenience function to create a new `AttachmentOps`.
    pub fn new(load: AttachmentLoadOp, store: AttachmentStoreOp) -> Self {
        AttachmentOps { load, store }
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
#[derive(Clone, Debug, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Attachment {
    /// Attachment format
    ///
    /// In the most cases `format` is not `None`. It should be only used for
    /// creating dummy renderpasses, which are used as placeholder for compatible
    /// renderpasses.
    pub format: Option<Format>,
    /// Number of samples.
    pub samples: image::NumSamples,
    /// Load and store operations of the attachment
    pub ops: AttachmentOps,
    /// Load and store operations of the stencil aspect, if any
    #[cfg_attr(feature = "serde", serde(default = "AttachmentOps::whatever"))]
    pub stencil_ops: AttachmentOps,
    /// Initial image layout of the renderpass.
    pub initial_layout: AttachmentLayout,
    /// Final image layouts of the renderpass.
    pub final_layout: AttachmentLayout,
}

impl Attachment {
    /// Returns true if this attachment has some clear operations. This is useful
    /// when starting a render pass, since there has to be a clear value provided.
    pub fn has_clears(&self) -> bool {
        self.ops.load == AttachmentLoadOp::Clear || self.stencil_ops.load == AttachmentLoadOp::Clear
    }
}

/// Index of an attachment within a framebuffer/renderpass,
pub type AttachmentId = usize;
/// Reference to an attachment by index and expected image layout.
pub type AttachmentRef = (AttachmentId, AttachmentLayout);
/// An AttachmentId that can be used instead of providing an attachment.
pub const ATTACHMENT_UNUSED: AttachmentId = !0;

/// Which other subpasses a particular subpass depends on.
#[derive(Copy, Clone, Debug, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SubpassRef {
    /// The subpass depends on something that was submitted to the
    /// queue before or after the render pass began.
    External,
    /// The subpass depends on another subpass with the given index,
    /// which must be less than or equal to the index of the current
    /// subpass. The index here refers to the corresponding
    /// `SubpassId` of a `Subpass`.
    Pass(usize),
}

/// Expresses a dependency between multiple subpasses. This is used
/// both to describe a source or destination subpass; data either
/// explicitly passes from this subpass to the next or from another
/// subpass into this one.
#[derive(Clone, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SubpassDependency {
    /// Source subpass this one depends on.
    pub src_subpass: SubpassRef,
    /// Destination subpass this one depends on.
    pub dst_subpass: SubpassRef,
    /// Source pipeline stage this subpass depends on.
    pub src_stage: PipelineStage,
    /// Destination pipeline stage this subpass depends on.
    pub dst_stage: PipelineStage,
    /// Resource source access this subpass depends on.
    pub src_access: image::Access,
    /// Resource destination access this subpass depends on.
    pub dst_access: image::Access,
}

/// Description of a subpass for renderpass creation.
#[derive(Debug)]
pub struct SubpassDesc<'a> {
    /// Which attachments will be used as color buffers.
    pub colors: &'a [AttachmentRef],
    /// Which attachments will be used as depth/stencil buffers.
    pub depth_stencil: Option<&'a AttachmentRef>,
    /// Which attachments will be used as input attachments.
    pub inputs: &'a [AttachmentRef],
    /// Which attachments will be used as resolve destinations.
    ///
    /// The number of resolve attachments may be zero or equal to the number of color attachments.
    /// At the end of a subpass the color attachment will be resolved to the corresponding
    /// resolve attachment. The resolve attachment must not be multisampled.
    pub resolves: &'a [AttachmentRef],
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
        self.index == other.index && self.main_pass as *const _ == other.main_pass as *const _
    }
}

impl<'a, B: Backend> Copy for Subpass<'a, B> {}
impl<'a, B: Backend> Eq for Subpass<'a, B> {}
