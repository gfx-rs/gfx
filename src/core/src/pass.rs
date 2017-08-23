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
use image::{ImageAccess, ImageLayout};
use pso::PipelineStage;
use Backend;

/// Specifies the operation which will be applied at the beginning of a subpass.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
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
pub enum AttachmentStoreOp {
    /// Content written to the attachment will be preserved.
    Store,
    /// Attachment content will be undefined.
    DontCare,
}

/// Image layout of an attachment.
pub type AttachmentLayout = ImageLayout;

///
#[derive(Clone, Debug, Hash)]
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
pub enum SubpassRef {
    ///
    External,
    ///
    Pass(usize),
}

/// Specifies dependencies between subpasses.
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
    pub src_access: ImageAccess,
    ///
    pub dst_access: ImageAccess,
}

/// Description of a subpass for renderpass creation.
pub struct SubpassDesc<'a> {
    ///
    pub color_attachments: &'a [AttachmentRef],
}

/// Reference to a subpass of a renderpass by index.
pub struct SubPass<'a, B: Backend> {
    ///
    pub index: usize,
    /// Parent renderpass.
    pub main_pass: &'a B::RenderPass,
}
