// Copyright 2014 The Gfx-rs Developers.
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

//! OpenGL implementation of the Command Buffer

use std::slice;

use gfx::device as d;
use gfx::device::handle;
use gfx::device::draw::{Access, Target};
use gfx::device::target::*;
use super::{ArrayBuffer, Buffer, FrameBuffer, Program, Surface, Texture, GlResources};

/// Serialized device command.
#[derive(Copy, Debug)]
pub enum Command {
    BindProgram(Program),
    BindArrayBuffer(ArrayBuffer),
    BindAttribute(d::AttributeSlot, Buffer, d::attrib::Format),
    BindIndex(Buffer),
    BindFrameBuffer(Access, FrameBuffer),
    UnbindTarget(Access, Target),
    BindTargetSurface(Access, Target, Surface),
    BindTargetTexture(Access, Target, Texture, Level, Option<Layer>),
    BindUniformBlock(Program, d::UniformBufferSlot, d::UniformBlockIndex, Buffer),
    BindUniform(d::shade::Location, d::shade::UniformValue),
    BindTexture(d::TextureSlot, d::tex::TextureKind, Texture,
                Option<handle::Sampler<GlResources>>),
    SetDrawColorBuffers(usize),
    SetPrimitiveState(d::state::Primitive),
    SetViewport(Rect),
    SetMultiSampleState(Option<d::state::MultiSample>),
    SetScissor(Option<Rect>),
    SetDepthStencilState(Option<d::state::Depth>, Option<d::state::Stencil>,
                         d::state::CullFace),
    SetBlendState(Option<d::state::Blend>),
    SetColorMask(d::state::ColorMask),
    UpdateBuffer(Buffer, d::draw::DataPointer, usize),
    UpdateTexture(d::tex::TextureKind, Texture, d::tex::ImageInfo, d::draw::DataPointer),
    // drawing
    Clear(ClearData, Mask),
    Draw(d::PrimitiveType, d::VertexCount, d::VertexCount, d::draw::InstanceOption),
    DrawIndexed(d::PrimitiveType, d::IndexType, d::VertexCount, d::VertexCount,
                d::VertexCount, d::draw::InstanceOption),
    Blit(Rect, Rect, Mirror, Mask),
}

pub struct CommandBuffer {
    buf: Vec<Command>,
}

impl CommandBuffer {
    pub fn iter<'a>(&'a self) -> slice::Iter<'a, Command> {
        self.buf.iter()
    }
}

impl d::draw::CommandBuffer<GlResources> for CommandBuffer {
    fn new() -> CommandBuffer {
        CommandBuffer {
            buf: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.buf.clear();
    }

    fn bind_program(&mut self, prog: Program) {
        self.buf.push(Command::BindProgram(prog));
    }

    fn bind_array_buffer(&mut self, vao: ArrayBuffer) {
        self.buf.push(Command::BindArrayBuffer(vao));
    }

    fn bind_attribute(&mut self, slot: d::AttributeSlot, buf: Buffer,
                      format: d::attrib::Format) {
        self.buf.push(Command::BindAttribute(slot, buf, format));
    }

    fn bind_index(&mut self, buf: Buffer) {
        self.buf.push(Command::BindIndex(buf));
    }

    fn bind_frame_buffer(&mut self, access: Access, fbo: FrameBuffer) {
        self.buf.push(Command::BindFrameBuffer(access, fbo));
    }

    fn unbind_target(&mut self, access: Access, tar: Target) {
        self.buf.push(Command::UnbindTarget(access, tar));
    }

    fn bind_target_surface(&mut self, access: Access, tar: Target, suf: Surface) {
        self.buf.push(Command::BindTargetSurface(access, tar, suf));
    }

    fn bind_target_texture(&mut self, access: Access, tar: Target, tex: Texture,
                           level: Level, layer: Option<Layer>) {
        self.buf.push(Command::BindTargetTexture(access, tar, tex, level, layer));
    }

    fn bind_uniform_block(&mut self, prog: Program, slot: d::UniformBufferSlot,
                          index: d::UniformBlockIndex, buf: Buffer) {
        self.buf.push(Command::BindUniformBlock(prog, slot, index, buf));
    }

    fn bind_uniform(&mut self, loc: d::shade::Location, value: d::shade::UniformValue) {
        self.buf.push(Command::BindUniform(loc, value));
    }
    fn bind_texture(&mut self, slot: d::TextureSlot, kind: d::tex::TextureKind,
                    tex: Texture, sampler: Option<handle::Sampler<GlResources>>) {
        self.buf.push(Command::BindTexture(slot, kind, tex, sampler));
    }

    fn set_draw_color_buffers(&mut self, num: usize) {
        self.buf.push(Command::SetDrawColorBuffers(num));
    }

    fn set_primitive(&mut self, prim: d::state::Primitive) {
        self.buf.push(Command::SetPrimitiveState(prim));
    }

    fn set_viewport(&mut self, view: Rect) {
        self.buf.push(Command::SetViewport(view));
    }

    fn set_multi_sample(&mut self, ms: Option<d::state::MultiSample>) {
        self.buf.push(Command::SetMultiSampleState(ms));
    }

    fn set_scissor(&mut self, rect: Option<Rect>) {
        self.buf.push(Command::SetScissor(rect));
    }

    fn set_depth_stencil(&mut self, depth: Option<d::state::Depth>,
                         stencil: Option<d::state::Stencil>,
                         cull: d::state::CullFace) {
        self.buf.push(Command::SetDepthStencilState(depth, stencil, cull));
    }

    fn set_blend(&mut self, blend: Option<d::state::Blend>) {
        self.buf.push(Command::SetBlendState(blend));
    }

    fn set_color_mask(&mut self, mask: d::state::ColorMask) {
        self.buf.push(Command::SetColorMask(mask));
    }

    fn update_buffer(&mut self, buf: Buffer, data: d::draw::DataPointer,
                        offset_bytes: usize) {
        self.buf.push(Command::UpdateBuffer(buf, data, offset_bytes));
    }

    fn update_texture(&mut self, kind: d::tex::TextureKind, tex: Texture,
                      info: d::tex::ImageInfo, data: d::draw::DataPointer) {
        self.buf.push(Command::UpdateTexture(kind, tex, info, data));
    }

    fn call_clear(&mut self, data: ClearData, mask: Mask) {
        self.buf.push(Command::Clear(data, mask));
    }

    fn call_draw(&mut self, ptype: d::PrimitiveType, start: d::VertexCount,
                 count: d::VertexCount, instances: d::draw::InstanceOption) {
        self.buf.push(Command::Draw(ptype, start, count, instances));
    }

    fn call_draw_indexed(&mut self, ptype: d::PrimitiveType, itype: d::IndexType,
                         start: d::VertexCount, count: d::VertexCount,
                         base: d::VertexCount, instances: d::draw::InstanceOption) {
        self.buf.push(Command::DrawIndexed(ptype, itype, start, count, base, instances));
    }

    fn call_blit(&mut self, s_rect: Rect, d_rect: Rect, mirror: Mirror, mask: Mask) {
        self.buf.push(Command::Blit(s_rect, d_rect, mirror, mask));
    }
}
