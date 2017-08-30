// Copyright 2017 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! RenderPass handling.

use format::Format;
use image;
use pso::PipelineStage;
use Backend;

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

///
#[derive(Clone, Debug, Hash)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Attachment {
    ///
    pub format: Format,
    ///
    pub load_op: AttachmentLoadOp,
    ///
    pub store_op: AttachmentStoreOp,
    ///
    pub stencil_load_op: AttachmentLoadOp,
    ///
    pub stencil_store_op:AttachmentStoreOp,
    /// Initial image layout in the beginning of the renderpass.
    pub src_layout: AttachmentLayout,
    /// Final image layout at the end of the renderpass.
    pub dst_layout: AttachmentLayout,
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
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct SubpassDependency {
    ///
    pub src_pass: SubpassRef,
    ///
    pub dst_pass: SubpassRef,
    ///
    pub src_stage: PipelineStage,
    ///
    pub dst_stage: PipelineStage,
    ///
    pub src_access: image::Access,
    ///
    pub dst_access: image::Access,
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
