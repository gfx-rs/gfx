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

//!

use format::Format;
use texture::{ImageAccess, ImageLayout};
use pso::PipelineStage;
use Backend;

///
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum AttachmentLoadOp {
    ///
    Load,
    ///
    Clear,
    ///
    DontCare,
}

///
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum AttachmentStoreOp {
    ///
    Store,
    ///
    DontCare,
}

///
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
    ///
    pub src_layout: AttachmentLayout,
    ///
    pub dst_layout: AttachmentLayout,
}

///
pub type AttachmentRef = (usize, AttachmentLayout);

///
#[derive(Copy, Clone, Debug)]
pub enum SubpassRef {
    ///
    External,
    ///
    Pass(usize),
}

///
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

///
pub struct SubpassDesc<'a> {
    ///
    pub color_attachments: &'a [AttachmentRef],
}

///
pub struct SubPass<'a, B: Backend> {
    ///
    pub index: usize,
    ///
    pub main_pass: &'a B::RenderPass,
}
