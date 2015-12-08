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
use draw_state::target::{ClearData, Layer, Level, Mask, Mirror, Rect};
use {attrib, draw, shade, state, pso, tex};
use {Resources, IndexType, VertexCount, Primitive,
     AttributeSlot, TextureSlot, ColorSlot, UniformBufferSlot, UniformBlockIndex};

///Generic command buffer to be used by multiple backends
pub struct CommandBuffer<R: Resources> {
    buf: Vec<Command<R>>
}

///Serialized device command.
#[derive(Copy, Clone, Debug)]
pub enum Command<R: Resources> {
    BindProgram(R::Program),
    BindPipelineState(R::PipelineStateObject),
    BindVertexBuffers(pso::VertexBufferSet<R>),
    BindConstantBuffers(pso::ConstantBufferSet<R>),
    BindPixelTargets(pso::PixelTargetSet<R>),
    BindArrayBuffer(R::ArrayBuffer),
    BindAttribute(AttributeSlot, R::Buffer, attrib::Format),
    BindIndex(R::Buffer),
    BindFrameBuffer(draw::Access, R::FrameBuffer, draw::Gamma),
    UnbindTarget(draw::Access, draw::Target),
    BindTargetSurface(draw::Access, draw::Target, R::Surface),
    BindTargetTexture(draw::Access, draw::Target, R::Texture, Level, Option<Layer>),
    BindUniformBlock(R::Program, UniformBufferSlot, UniformBlockIndex,
                     R::Buffer),
    BindUniform(shade::Location, shade::UniformValue),
    BindTexture(TextureSlot, tex::Kind, R::Texture,
                Option<(R::Sampler, tex::SamplerInfo)>),
    SetDrawColorBuffers(ColorSlot),
    SetRasterizer(state::Rasterizer),
    SetViewport(Rect),
    SetScissor(Option<Rect>),
    SetDepthStencilState(Option<state::Depth>, Option<state::Stencil>,
                         state::CullFace),
    SetBlendState(ColorSlot, Option<state::Blend>),
    SetRefValues(state::RefValues),
    UpdateBuffer(R::Buffer, draw::DataPointer, usize),
    UpdateTexture(tex::Kind, R::Texture, tex::ImageInfo,
                  draw::DataPointer),
    // drawing
    Clear(ClearData, Mask),
    Draw(Primitive, VertexCount, VertexCount,
         draw::InstanceOption),
    DrawIndexed(Primitive, IndexType, VertexCount, VertexCount,
                VertexCount, draw::InstanceOption),
    Blit(Rect, Rect, Mirror, Mask),
}

impl<R> CommandBuffer<R> where R: Resources {
    pub fn iter<'a>(&'a self) -> slice::Iter<'a, Command<R>> {
        self.buf.iter()
    }
}

impl<R> draw::CommandBuffer<R> for CommandBuffer<R>
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

    fn bind_pipeline_state(&mut self, pso: R::PipelineStateObject) {
        self.buf.push(Command::BindPipelineState(pso));
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<R>) {
        self.buf.push(Command::BindVertexBuffers(vbs));
    }

    fn bind_constant_buffers(&mut self, cbs: pso::ConstantBufferSet<R>) {
        self.buf.push(Command::BindConstantBuffers(cbs));
    }

    fn bind_pixel_targets(&mut self, pts: pso::PixelTargetSet<R>) {
        self.buf.push(Command::BindPixelTargets(pts));
    }

    fn bind_array_buffer(&mut self, vao: R::ArrayBuffer) {
        self.buf.push(Command::BindArrayBuffer(vao));
    }

    fn bind_attribute(&mut self, slot: AttributeSlot, buf: R::Buffer,
                      format: attrib::Format) {
        self.buf.push(Command::BindAttribute(slot, buf, format));
    }

    fn bind_index(&mut self, buf: R::Buffer) {
        self.buf.push(Command::BindIndex(buf));
    }

    fn bind_frame_buffer(&mut self, access: draw::Access, fbo: R::FrameBuffer,
                         gamma: draw::Gamma) {
        self.buf.push(Command::BindFrameBuffer(access, fbo, gamma));
    }

    fn unbind_target(&mut self, access: draw::Access, tar: draw::Target) {
        self.buf.push(Command::UnbindTarget(access, tar));
    }

    fn bind_target_surface(&mut self, access: draw::Access, tar: draw::Target,
                           suf: R::Surface) {
        self.buf.push(Command::BindTargetSurface(access, tar, suf));
    }

    fn bind_target_texture(&mut self, access: draw::Access, tar: draw::Target,
                           tex: R::Texture, level: Level, layer: Option<Layer>) {
        self.buf.push(Command::BindTargetTexture(
            access, tar, tex, level, layer));
    }

    fn bind_uniform_block(&mut self, prog: R::Program, slot: UniformBufferSlot,
                          index: UniformBlockIndex, buf: R::Buffer) {
        self.buf.push(Command::BindUniformBlock(prog, slot, index, buf));
    }

    fn bind_uniform(&mut self, loc: shade::Location,
                    value: shade::UniformValue) {
        self.buf.push(Command::BindUniform(loc, value));
    }

    fn bind_texture(&mut self, slot: TextureSlot, kind: tex::Kind, tex: R::Texture,
                    sampler: Option<(R::Sampler, tex::SamplerInfo)>) {
        self.buf.push(Command::BindTexture(slot, kind, tex, sampler));
    }

    fn set_draw_color_buffers(&mut self, num: ColorSlot) {
        self.buf.push(Command::SetDrawColorBuffers(num));
    }

    fn set_rasterizer(&mut self, rast: state::Rasterizer) {
        self.buf.push(Command::SetRasterizer(rast));
    }

    fn set_viewport(&mut self, view: Rect) {
        self.buf.push(Command::SetViewport(view));
    }

    fn set_scissor(&mut self, rect: Option<Rect>) {
        self.buf.push(Command::SetScissor(rect));
    }

    fn set_depth_stencil(&mut self, depth: Option<state::Depth>,
                         stencil: Option<state::Stencil>,
                         cull: state::CullFace) {
        self.buf.push(Command::SetDepthStencilState(depth, stencil, cull));
    }

    fn set_blend(&mut self, slot: ColorSlot, blend: Option<state::Blend>) {
        self.buf.push(Command::SetBlendState(slot, blend));
    }

    fn set_ref_values(&mut self, rv: state::RefValues) {
        self.buf.push(Command::SetRefValues(rv));
    }

    fn update_buffer(&mut self, buf: R::Buffer, data: draw::DataPointer,
                        offset_bytes: usize) {
        self.buf.push(Command::UpdateBuffer(buf, data, offset_bytes));
    }

    fn update_texture(&mut self, kind: tex::Kind, tex: R::Texture,
                      info: tex::ImageInfo, data: draw::DataPointer) {
        self.buf.push(Command::UpdateTexture(kind, tex, info, data));
    }

    fn call_clear(&mut self, data: ClearData, mask: Mask) {
        self.buf.push(Command::Clear(data, mask));
    }

    fn call_draw(&mut self, prim: Primitive, start: VertexCount,
                 count: VertexCount, instances: draw::InstanceOption) {
        self.buf.push(Command::Draw(prim, start, count, instances));
    }

    fn call_draw_indexed(&mut self, prim: Primitive,
                         itype: IndexType, start: VertexCount,
                         count: VertexCount, base: VertexCount,
                         instances: draw::InstanceOption) {
        self.buf.push(Command::DrawIndexed(
            prim, itype, start, count, base, instances));
    }

    fn call_blit(&mut self, s_rect: Rect, d_rect: Rect, mirror: Mirror,
                 mask: Mask) {
        self.buf.push(Command::Blit(s_rect, d_rect, mirror, mask));
    }
}
