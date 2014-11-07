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

pub struct GlCommandBuffer {
    buf: Vec<::Command>,
}

impl GlCommandBuffer {
    pub fn iter<'a>(&'a self) -> slice::Items<'a, ::Command> {
        self.buf.iter()
    }
}

impl ::draw::CommandBuffer for GlCommandBuffer {
    fn new() -> GlCommandBuffer {
        GlCommandBuffer {
            buf: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.buf.clear();
    }

    fn bind_program(&mut self, prog: super::Program) {
        self.buf.push(::BindProgram(prog));
    }

    fn bind_array_buffer(&mut self, vao: super::ArrayBuffer) {
        self.buf.push(::BindArrayBuffer(vao));
    }

    fn bind_attribute(&mut self, slot: ::AttributeSlot, buf: super::Buffer,
                      format: ::attrib::Format) {
        self.buf.push(::BindAttribute(slot, buf, format));
    }

    fn bind_index(&mut self, buf: super::Buffer) {
        self.buf.push(::BindIndex(buf));
    }

    fn bind_frame_buffer(&mut self, access: ::target::Access, fbo: super::FrameBuffer) {
        self.buf.push(::BindFrameBuffer(access, fbo));
    }

    fn unbind_target(&mut self, access: ::target::Access, tar: ::target::Target) {
        self.buf.push(::UnbindTarget(access, tar));
    }

    fn bind_target_surface(&mut self, access: ::target::Access,
                           tar: ::target::Target, suf: super::Surface) {
        self.buf.push(::BindTargetSurface(access, tar, suf));
    }

    fn bind_target_texture(&mut self, access: ::target::Access,
                           tar: ::target::Target, tex: super::Texture,
                           level: ::target::Level, layer: Option<::target::Layer>) {
        self.buf.push(::BindTargetTexture(access, tar, tex, level, layer));
    }

    fn bind_uniform_block(&mut self, prog: super::Program, slot: ::UniformBufferSlot,
                          index: ::UniformBlockIndex, buf: super::Buffer) {
        self.buf.push(::BindUniformBlock(prog, slot, index, buf));
    }

    fn bind_uniform(&mut self, loc: ::shade::Location, value: ::shade::UniformValue) {
        self.buf.push(::BindUniform(loc, value));
    }
    fn bind_texture(&mut self, slot: ::TextureSlot, kind: ::tex::TextureKind,
                    tex: super::Texture, sampler: Option<::SamplerHandle>) {
        self.buf.push(::BindTexture(slot, kind, tex, sampler));
    }

    fn set_primitive(&mut self, prim: ::state::Primitive) {
        self.buf.push(::SetPrimitiveState(prim));
    }

    fn set_viewport(&mut self, view: ::target::Rect) {
        self.buf.push(::SetViewport(view));
    }

    fn set_multi_sample(&mut self, ms: Option<::state::MultiSample>) {
        self.buf.push(::SetMultiSampleState(ms));
    }

    fn set_scissor(&mut self, rect: Option<::target::Rect>) {
        self.buf.push(::SetScissor(rect));
    }

    fn set_depth_stencil(&mut self, depth: Option<::state::Depth>,
                         stencil: Option<::state::Stencil>, cull: ::state::CullMode) {
        self.buf.push(::SetDepthStencilState(depth, stencil, cull));
    }

    fn set_blend(&mut self, blend: Option<::state::Blend>) {
        self.buf.push(::SetBlendState(blend));
    }

    fn set_color_mask(&mut self, mask: ::state::ColorMask) {
        self.buf.push(::SetColorMask(mask));
    }

    fn update_buffer(&mut self, buf: super::Buffer, data: ::draw::DataPointer,
                        offset_bytes: uint) {
        self.buf.push(::UpdateBuffer(buf, data, offset_bytes));
    }

    fn update_texture(&mut self, kind: ::tex::TextureKind, tex: super::Texture,
                      info: ::tex::ImageInfo, data: ::draw::DataPointer) {
        self.buf.push(::UpdateTexture(kind, tex, info, data));
    }

    fn call_clear(&mut self, data: ::target::ClearData, mask: ::target::Mask) {
        self.buf.push(::Clear(data, mask));
    }

    fn call_draw(&mut self, ptype: ::PrimitiveType, start: ::VertexCount,
                 count: ::VertexCount, instances: Option<::InstanceCount>) {
        self.buf.push(::Draw(ptype, start, count, instances));
    }

    fn call_draw_indexed(&mut self, ptype: ::PrimitiveType, itype: ::IndexType,
                         start: ::VertexCount, count: ::VertexCount, base: ::VertexCount,
                         instances: Option<::InstanceCount>) {
        self.buf.push(::DrawIndexed(ptype, itype, start, count, base, instances));
    }

    fn call_blit(&mut self, s_rect: ::target::Rect, d_rect: ::target::Rect,
                 mask: ::target::Mask) {
        self.buf.push(::Blit(s_rect, d_rect, mask));
    }
}
