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

use gfx_core as c;
use gfx_core::draw::{Access, Gamma, Target, DataPointer, InstanceOption};
use gfx_core::state as s;
use gfx_core::target::{ClearData, Layer, Level, Mask, Mirror, Rect};
use {Buffer, ArrayBuffer, Program, FrameBuffer, Surface, Sampler, Texture,
     Resources, PipelineState};

///Serialized device command.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    BindProgram(Program),
    BindPipelineState(PipelineState),
    BindVertexBuffers(c::pso::VertexBufferSet<Resources>),
    BindConstantBuffers(c::pso::ConstantBufferSet<Resources>),
    BindPixelTargets(c::pso::PixelTargetSet<Resources>),
    BindArrayBuffer(ArrayBuffer),
    BindAttribute(c::AttributeSlot, Buffer, c::attrib::Format),
    BindIndex(Buffer),
    BindFrameBuffer(Access, FrameBuffer, Gamma),
    UnbindTarget(Access, Target),
    BindTargetSurface(Access, Target, Surface),
    BindTargetTexture(Access, Target, Texture,
                      Level, Option<Layer>),
    BindUniformBlock(Program, c::UniformBufferSlot, c::UniformBlockIndex,
                     Buffer),
    BindUniform(c::shade::Location, c::shade::UniformValue),
    BindTexture(c::TextureSlot, c::tex::Kind, Texture,
                Option<(Sampler, c::tex::SamplerInfo)>),
    SetDrawColorBuffers(c::ColorSlot),
    SetRasterizer(s::Rasterizer),
    SetViewport(Rect),
    SetScissor(Option<Rect>),
    SetDepthStencilState(Option<s::Depth>, Option<s::Stencil>, s::CullFace),
    SetBlendState(c::ColorSlot, Option<s::Blend>),
    SetRefValues(s::RefValues),
    UpdateBuffer(Buffer, DataPointer, usize),
    UpdateTexture(c::tex::Kind, Texture, c::tex::ImageInfo, DataPointer),
    // drawing
    Clear(ClearData, Mask),
    Draw(c::Primitive, c::VertexCount, c::VertexCount, InstanceOption),
    DrawIndexed(c::Primitive, c::IndexType, c::VertexCount, c::VertexCount,
                c::VertexCount, InstanceOption),
    Blit(Rect, Rect, Mirror, Mask),
}

pub struct CommandBuffer {
    pub buf: Vec<Command>,
}

impl c::draw::CommandBuffer<Resources> for CommandBuffer {
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

    fn bind_pipeline_state(&mut self, pso: PipelineState) {
        self.buf.push(Command::BindPipelineState(pso));
    }

    fn bind_vertex_buffers(&mut self, vbs: c::pso::VertexBufferSet<Resources>) {
        self.buf.push(Command::BindVertexBuffers(vbs));
    }

    fn bind_constant_buffers(&mut self, cbs: c::pso::ConstantBufferSet<Resources>) {
        self.buf.push(Command::BindConstantBuffers(cbs));
    }

    fn bind_pixel_targets(&mut self, pts: c::pso::PixelTargetSet<Resources>) {
        self.buf.push(Command::BindPixelTargets(pts));
    }

    fn bind_array_buffer(&mut self, vao: ArrayBuffer) {
        self.buf.push(Command::BindArrayBuffer(vao));
    }

    fn bind_attribute(&mut self, slot: c::AttributeSlot, buf: Buffer,
                      format: c::attrib::Format) {
        self.buf.push(Command::BindAttribute(slot, buf, format));
    }

    fn bind_index(&mut self, buf: Buffer) {
        self.buf.push(Command::BindIndex(buf));
    }

    fn bind_frame_buffer(&mut self, access: Access, fbo: FrameBuffer,
                         gamma: Gamma) {
        self.buf.push(Command::BindFrameBuffer(access, fbo, gamma));
    }

    fn unbind_target(&mut self, access: Access, tar: Target) {
        self.buf.push(Command::UnbindTarget(access, tar));
    }

    fn bind_target_surface(&mut self, access: Access, tar: Target,
                           suf: Surface) {
        self.buf.push(Command::BindTargetSurface(access, tar, suf));
    }

    fn bind_target_texture(&mut self, access: Access, tar: Target,
                           tex: Texture, level: Level, layer: Option<Layer>) {
        self.buf.push(Command::BindTargetTexture(
            access, tar, tex, level, layer));
    }

    fn bind_uniform_block(&mut self, prog: Program, slot: c::UniformBufferSlot,
                          index: c::UniformBlockIndex, buf: Buffer) {
        self.buf.push(Command::BindUniformBlock(prog, slot, index, buf));
    }

    fn bind_uniform(&mut self, loc: c::shade::Location,
                    value: c::shade::UniformValue) {
        self.buf.push(Command::BindUniform(loc, value));
    }

    fn bind_texture(&mut self, slot: c::TextureSlot, kind: c::tex::Kind, tex: Texture,
                    sampler: Option<(Sampler, c::tex::SamplerInfo)>) {
        self.buf.push(Command::BindTexture(slot, kind, tex, sampler));
    }

    fn set_draw_color_buffers(&mut self, num: c::ColorSlot) {
        self.buf.push(Command::SetDrawColorBuffers(num));
    }

    fn set_rasterizer(&mut self, rast: s::Rasterizer) {
        self.buf.push(Command::SetRasterizer(rast));
    }

    fn set_viewport(&mut self, view: Rect) {
        self.buf.push(Command::SetViewport(view));
    }

    fn set_scissor(&mut self, rect: Option<Rect>) {
        self.buf.push(Command::SetScissor(rect));
    }

    fn set_depth_stencil(&mut self, depth: Option<s::Depth>,
                         stencil: Option<s::Stencil>,
                         cull: s::CullFace) {
        self.buf.push(Command::SetDepthStencilState(depth, stencil, cull));
    }

    fn set_blend(&mut self, slot: c::ColorSlot, blend: Option<s::Blend>) {
        self.buf.push(Command::SetBlendState(slot, blend));
    }

    fn set_ref_values(&mut self, rv: s::RefValues) {
        self.buf.push(Command::SetRefValues(rv));
    }

    fn update_buffer(&mut self, buf: Buffer, data: DataPointer,
                        offset_bytes: usize) {
        self.buf.push(Command::UpdateBuffer(buf, data, offset_bytes));
    }

    fn update_texture(&mut self, kind: c::tex::Kind, tex: Texture,
                      info: c::tex::ImageInfo, data: DataPointer) {
        self.buf.push(Command::UpdateTexture(kind, tex, info, data));
    }

    fn call_clear(&mut self, data: ClearData, mask: Mask) {
        self.buf.push(Command::Clear(data, mask));
    }

    fn call_draw(&mut self, prim: c::Primitive, start: c::VertexCount,
                 count: c::VertexCount, instances: InstanceOption) {
        self.buf.push(Command::Draw(prim, start, count, instances));
    }

    fn call_draw_indexed(&mut self, prim: c::Primitive,
                         itype: c::IndexType, start: c::VertexCount,
                         count: c::VertexCount, base: c::VertexCount,
                         instances: InstanceOption) {
        self.buf.push(Command::DrawIndexed(
            prim, itype, start, count, base, instances));
    }

    fn call_blit(&mut self, s_rect: Rect, d_rect: Rect, mirror: Mirror,
                 mask: Mask) {
        self.buf.push(Command::Blit(s_rect, d_rect, mirror, mask));
    }
}
