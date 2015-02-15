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

use {attrib, back, draw, target, tex, shade, state};
use {AttributeSlot, IndexType, InstanceCount, PrimitiveType, TextureSlot, UniformBlockIndex, UniformBufferSlot, VertexCount};
use super::{ArrayBuffer, Buffer, FrameBuffer, Program, Sampler, Surface, Texture};

/// Serialized device command.
#[derive(Copy, Debug)]
pub enum Command {
    BindProgram(Program),
    BindArrayBuffer(ArrayBuffer),
    BindAttribute(AttributeSlot, Buffer, attrib::Format),
    BindIndex(Buffer),
    BindFrameBuffer(target::Access, FrameBuffer),
    UnbindTarget(target::Access, target::Target),
    BindTargetSurface(target::Access, target::Target, Surface),
    BindTargetTexture(target::Access, target::Target, Texture, target::Level, Option<target::Layer>),
    BindUniformBlock(Program, UniformBufferSlot, UniformBlockIndex, Buffer),
    BindUniform(shade::Location, shade::UniformValue),
    BindTexture(TextureSlot, tex::TextureKind, Texture, Option<::SamplerHandle<back::GlDevice>>),
    SetDrawColorBuffers(usize),
    SetPrimitiveState(state::Primitive),
    SetViewport(target::Rect),
    SetMultiSampleState(Option<state::MultiSample>),
    SetScissor(Option<target::Rect>),
    SetDepthStencilState(Option<state::Depth>, Option<state::Stencil>, state::CullMode),
    SetBlendState(Option<state::Blend>),
    SetColorMask(state::ColorMask),
    UpdateBuffer(Buffer, draw::DataPointer, usize),
    UpdateTexture(tex::TextureKind, Texture, tex::ImageInfo, draw::DataPointer),
    // drawing
    Clear(target::ClearData, target::Mask),
    Draw(PrimitiveType, VertexCount, VertexCount, Option<(InstanceCount, VertexCount)>),
    DrawIndexed(PrimitiveType, IndexType, VertexCount, VertexCount, VertexCount, Option<(InstanceCount, VertexCount)>),
    Blit(target::Rect, target::Rect, target::Mirror, target::Mask),
}

pub struct CommandBuffer {
    buf: Vec<Command>,
}

impl CommandBuffer {
    pub fn iter<'a>(&'a self) -> slice::Iter<'a, Command> {
        self.buf.iter()
    }
}

impl draw::CommandBuffer for CommandBuffer {
    type Buffer         = Buffer;
    type ArrayBuffer    = ArrayBuffer;
    type Program        = Program;
    type FrameBuffer    = FrameBuffer;
    type Surface        = Surface;
    type Texture        = Texture;
    type Sampler        = Sampler;

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

    fn bind_attribute(&mut self, slot: ::AttributeSlot, buf: Buffer,
                      format: ::attrib::Format) {
        self.buf.push(Command::BindAttribute(slot, buf, format));
    }

    fn bind_index(&mut self, buf: Buffer) {
        self.buf.push(Command::BindIndex(buf));
    }

    fn bind_frame_buffer(&mut self, access: target::Access, fbo: FrameBuffer) {
        self.buf.push(Command::BindFrameBuffer(access, fbo));
    }

    fn unbind_target(&mut self, access: target::Access, tar: target::Target) {
        self.buf.push(Command::UnbindTarget(access, tar));
    }

    fn bind_target_surface(&mut self, access: target::Access,
                           tar: target::Target, suf: Surface) {
        self.buf.push(Command::BindTargetSurface(access, tar, suf));
    }

    fn bind_target_texture(&mut self, access: target::Access,
                           tar: target::Target, tex: Texture,
                           level: target::Level, layer: Option<target::Layer>) {
        self.buf.push(Command::BindTargetTexture(access, tar, tex, level, layer));
    }

    fn bind_uniform_block(&mut self, prog: Program, slot: ::UniformBufferSlot,
                          index: ::UniformBlockIndex, buf: Buffer) {
        self.buf.push(Command::BindUniformBlock(prog, slot, index, buf));
    }

    fn bind_uniform(&mut self, loc: ::shade::Location, value: ::shade::UniformValue) {
        self.buf.push(Command::BindUniform(loc, value));
    }
    fn bind_texture(&mut self, slot: ::TextureSlot, kind: ::tex::TextureKind,
                    tex: Texture, sampler: Option<::SamplerHandle<back::GlDevice>>) {
        self.buf.push(Command::BindTexture(slot, kind, tex, sampler));
    }

    fn set_draw_color_buffers(&mut self, num: usize) {
        self.buf.push(Command::SetDrawColorBuffers(num));
    }

    fn set_primitive(&mut self, prim: state::Primitive) {
        self.buf.push(Command::SetPrimitiveState(prim));
    }

    fn set_viewport(&mut self, view: target::Rect) {
        self.buf.push(Command::SetViewport(view));
    }

    fn set_multi_sample(&mut self, ms: Option<state::MultiSample>) {
        self.buf.push(Command::SetMultiSampleState(ms));
    }

    fn set_scissor(&mut self, rect: Option<target::Rect>) {
        self.buf.push(Command::SetScissor(rect));
    }

    fn set_depth_stencil(&mut self, depth: Option<state::Depth>,
                         stencil: Option<state::Stencil>, cull: state::CullMode) {
        self.buf.push(Command::SetDepthStencilState(depth, stencil, cull));
    }

    fn set_blend(&mut self, blend: Option<state::Blend>) {
        self.buf.push(Command::SetBlendState(blend));
    }

    fn set_color_mask(&mut self, mask: state::ColorMask) {
        self.buf.push(Command::SetColorMask(mask));
    }

    fn update_buffer(&mut self, buf: Buffer, data: draw::DataPointer,
                        offset_bytes: usize) {
        self.buf.push(Command::UpdateBuffer(buf, data, offset_bytes));
    }

    fn update_texture(&mut self, kind: ::tex::TextureKind, tex: Texture,
                      info: ::tex::ImageInfo, data: draw::DataPointer) {
        self.buf.push(Command::UpdateTexture(kind, tex, info, data));
    }

    fn call_clear(&mut self, data: target::ClearData, mask: target::Mask) {
        self.buf.push(Command::Clear(data, mask));
    }

    fn call_draw(&mut self, ptype: ::PrimitiveType, start: ::VertexCount,
                 count: ::VertexCount, instances: Option<(::InstanceCount, ::VertexCount)>) {
        self.buf.push(Command::Draw(ptype, start, count, instances));
    }

    fn call_draw_indexed(&mut self, ptype: ::PrimitiveType, itype: ::IndexType,
                         start: ::VertexCount, count: ::VertexCount, base: ::VertexCount,
                         instances: Option<(::InstanceCount, ::VertexCount)>) {
        self.buf.push(Command::DrawIndexed(ptype, itype, start, count, base, instances));
    }

    fn call_blit(&mut self, s_rect: target::Rect, d_rect: target::Rect,
                 mirror: target::Mirror, mask: target::Mask) {
        self.buf.push(Command::Blit(s_rect, d_rect, mirror, mask));
    }
}
