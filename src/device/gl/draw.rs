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

//! OpenGL implementation of the `DrawList`

use std::slice;

pub struct DrawList {
	buf: Vec<::Command>,
}

impl DrawList {
	pub fn new() -> DrawList {
		DrawList {
			buf: Vec::new(),
		}
	}

	pub fn iter<'a>(&'a self) -> slice::Items<'a, ::Command> {
		self.buf.iter()
	}
}

impl ::draw::DrawList for DrawList {
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
					  count: ::attrib::Count, atype: ::attrib::Type,
					  stride: ::attrib::Stride, offset: ::attrib::Offset) {
		self.buf.push(::BindAttribute(slot, buf, count, atype, stride, offset));
	}

	fn bind_index(&mut self, buf: super::Buffer) {
		self.buf.push(::BindIndex(buf));
	}

	fn bind_frame_buffer(&mut self, fbo: super::FrameBuffer) {
		self.buf.push(::BindFrameBuffer(fbo));
	}

	fn unbind_target(&mut self, tar: ::target::Target) {
		self.buf.push(::UnbindTarget(tar));
	}

	fn bind_target_surface(&mut self, tar: ::target::Target, suf: super::Surface) {
		self.buf.push(::BindTargetSurface(tar, suf));
	}

	fn bind_target_texture(&mut self, tar: ::target::Target, tex: super::Texture,
						   level: ::target::Level, layer: Option<::target::Layer>) {
		self.buf.push(::BindTargetTexture(tar, tex, level, layer));
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

	fn update_buffer(&mut self, buf: super::Buffer, data: Box<::Blob + Send>) {
		self.buf.push(::UpdateBuffer(buf, data));
	}

	fn update_texture(&mut self, kind: ::tex::TextureKind, tex: super::Texture,
					  info: ::tex::ImageInfo, data: Box<::Blob + Send>) {
		self.buf.push(::UpdateTexture(kind, tex, info, data));
	}

	fn call_clear(&mut self, data: ::target::ClearData) {
		self.buf.push(::Clear(data));
	}

	fn call_draw(&mut self, ptype: ::PrimitiveType, start: ::VertexCount,
				 count: ::VertexCount) {
		self.buf.push(::Draw(ptype, start, count));
	}

	fn call_draw_indexed(&mut self, ptype: ::PrimitiveType, itype: ::IndexType,
						 start: ::IndexCount, count: ::IndexCount) {
		self.buf.push(::DrawIndexed(ptype, itype, start, count));
	}
}
