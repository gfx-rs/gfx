// Copyright 2015 The Gfx-rs Developers.
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
#![allow(missing_docs)]
use std::slice;

use device as d;
use device::{Resources};
use device::draw::{Access, Gamma, Target};
use draw_state::target::{ClearData, ColorValue, Layer, Level, Mask, Mirror, Rect, Stencil};

///Generic command buffer to be used by multiple backends
pub struct CommandBuffer<R: Resources> {
    buf: Vec<Command<R>>
}

///Serialized device command.
#[derive(Copy, Clone, Debug)]
pub enum Command<R: Resources> {
    BindProgram(R::Program),
    BindPipelineState(R::PipelineState),
    BindVertexBuffers(d::pso::VertexBufferSet<R>),
    BindArrayBuffer(R::ArrayBuffer),
    BindAttribute(d::AttributeSlot, R::Buffer, d::attrib::Format),
    BindIndex(R::Buffer),
    BindFrameBuffer(Access, R::FrameBuffer, Gamma),
    UnbindTarget(Access, Target),
    BindTargetSurface(Access, Target, R::Surface),
    BindTargetTexture(Access, Target, R::Texture, Level, Option<Layer>),
    BindUniformBlock(R::Program, d::UniformBufferSlot, d::UniformBlockIndex,
                     R::Buffer),
    BindUniform(d::shade::Location, d::shade::UniformValue),
    BindTexture(d::TextureSlot, d::tex::Kind, R::Texture,
                Option<(R::Sampler, d::tex::SamplerInfo)>),
    SetDrawColorBuffers(d::ColorSlot),
    SetPrimitiveState(d::state::Primitive),
    SetViewport(Rect),
    SetMultiSampleState(Option<d::state::MultiSample>),
    SetScissor(Option<Rect>),
    SetDepthStencilState(Option<d::state::Depth>, Option<d::state::Stencil>,
                         d::state::CullFace),
    SetBlendState(d::ColorSlot, Option<d::state::Blend>),
    SetRefValues(ColorValue, Stencil, Stencil),
    UpdateBuffer(R::Buffer, d::draw::DataPointer, usize),
    UpdateTexture(d::tex::Kind, R::Texture, d::tex::ImageInfo,
                  d::draw::DataPointer),
    // drawing
    Clear(ClearData, Mask),
    Draw(d::PrimitiveType, d::VertexCount, d::VertexCount,
         d::draw::InstanceOption),
    DrawIndexed(d::PrimitiveType, d::IndexType, d::VertexCount, d::VertexCount,
                d::VertexCount, d::draw::InstanceOption),
    Blit(Rect, Rect, Mirror, Mask),
}

impl<R> CommandBuffer<R> where R: Resources {
    pub fn iter<'a>(&'a self) -> slice::Iter<'a, Command<R>> {
        self.buf.iter()
    }
}

impl<R> d::draw::CommandBuffer<R> for CommandBuffer<R>
        where R : Resources {

    fn new() -> CommandBuffer<R> {
        CommandBuffer {
            buf: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.buf.clear();
    }

    fn bind_program(&mut self, prog: R::Program) {
        self.buf.push(Command::BindProgram(prog));
    }

    fn bind_pipeline_state(&mut self, pso: R::PipelineState) {
        self.buf.push(Command::BindPipelineState(pso));
    }

    fn bind_vertex_buffers(&mut self, vbs: d::pso::VertexBufferSet<R>) {
        self.buf.push(Command::BindVertexBuffers(vbs));
    }

    fn bind_array_buffer(&mut self, vao: R::ArrayBuffer) {
        self.buf.push(Command::BindArrayBuffer(vao));
    }

    fn bind_attribute(&mut self, slot: d::AttributeSlot, buf: R::Buffer,
                      format: d::attrib::Format) {
        self.buf.push(Command::BindAttribute(slot, buf, format));
    }

    fn bind_index(&mut self, buf: R::Buffer) {
        self.buf.push(Command::BindIndex(buf));
    }

    fn bind_frame_buffer(&mut self, access: Access, fbo: R::FrameBuffer,
                         gamma: Gamma) {
        self.buf.push(Command::BindFrameBuffer(access, fbo, gamma));
    }

    fn unbind_target(&mut self, access: Access, tar: Target) {
        self.buf.push(Command::UnbindTarget(access, tar));
    }

    fn bind_target_surface(&mut self, access: Access, tar: Target,
                           suf: R::Surface) {
        self.buf.push(Command::BindTargetSurface(access, tar, suf));
    }

    fn bind_target_texture(&mut self, access: Access, tar: Target,
                           tex: R::Texture, level: Level,
                           layer: Option<Layer>) {
        self.buf.push(Command::BindTargetTexture(
            access, tar, tex, level, layer));
    }

    fn bind_uniform_block(&mut self, prog: R::Program,
                          slot: d::UniformBufferSlot,
                          index: d::UniformBlockIndex, buf: R::Buffer) {
        self.buf.push(Command::BindUniformBlock(prog, slot, index, buf));
    }

    fn bind_uniform(&mut self, loc: d::shade::Location,
                    value: d::shade::UniformValue) {
        self.buf.push(Command::BindUniform(loc, value));
    }

    fn bind_texture(&mut self, slot: d::TextureSlot, kind: d::tex::Kind,
                    tex: R::Texture,
                    sampler: Option<(R::Sampler, d::tex::SamplerInfo)>) {
        self.buf.push(Command::BindTexture(slot, kind, tex, sampler));
    }

    fn set_draw_color_buffers(&mut self, num: d::ColorSlot) {
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

    fn set_blend(&mut self, slot: d::ColorSlot, blend: Option<d::state::Blend>) {
        self.buf.push(Command::SetBlendState(slot, blend));
    }

    fn set_ref_values(&mut self, blend: ColorValue, stencil_front: Stencil, stencil_back: Stencil) {
        self.buf.push(Command::SetRefValues(blend, stencil_front, stencil_back));
    }

    fn update_buffer(&mut self, buf: R::Buffer, data: d::draw::DataPointer,
                        offset_bytes: usize) {
        self.buf.push(Command::UpdateBuffer(buf, data, offset_bytes));
    }

    fn update_texture(&mut self, kind: d::tex::Kind, tex: R::Texture,
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

    fn call_draw_indexed(&mut self, ptype: d::PrimitiveType,
                         itype: d::IndexType, start: d::VertexCount,
                         count: d::VertexCount, base: d::VertexCount,
                         instances: d::draw::InstanceOption) {
        self.buf.push(Command::DrawIndexed(
            ptype, itype, start, count, base, instances));
    }

    fn call_blit(&mut self, s_rect: Rect, d_rect: Rect, mirror: Mirror,
                 mask: Mask) {
        self.buf.push(Command::Blit(s_rect, d_rect, mirror, mask));
    }
}
