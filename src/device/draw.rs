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

//! Draw List device interface

use a = attrib;
use b = back;
use t = target;

#[allow(missing_doc)]	//TODO
pub trait DrawList {
	/// Clear the draw list contents, retain the allocated storage
	fn clear(&mut self);
	fn bind_program(&mut self, b::Program);
	fn bind_array_buffer(&mut self, b::ArrayBuffer);
	fn bind_attribute(&mut self, ::AttributeSlot, b::Buffer, a::Count,
					  a::Type, a::Stride, a::Offset);
	fn bind_index(&mut self, b::Buffer);
	fn bind_frame_buffer(&mut self, b::FrameBuffer);
	/// Unbind any surface from the specified target slot
	fn unbind_target(&mut self, t::Target);
	/// Bind a surface to the specified target slot
	fn bind_target_surface(&mut self, t::Target, b::Surface);
	/// Bind a level of the texture to the specified target slot
	fn bind_target_texture(&mut self, t::Target, b::Texture,
						   t::Level, Option<t::Layer>);
	fn bind_uniform_block(&mut self, b::Program, ::UniformBufferSlot,
						  ::UniformBlockIndex, b::Buffer);
	fn bind_uniform(&mut self, ::shade::Location, ::shade::UniformValue);
	fn bind_texture(&mut self, ::TextureSlot, ::tex::TextureKind, b::Texture,
					Option<::SamplerHandle>);
	fn set_primitive(&mut self, ::state::Primitive);
	fn set_viewport(&mut self, t::Rect);
	fn set_scissor(&mut self, Option<t::Rect>);
	fn set_depth_stencil(&mut self, Option<::state::Depth>,
						 Option<::state::Stencil>, ::state::CullMode);
	fn set_blend(&mut self, Option<::state::Blend>);
	fn set_color_mask(&mut self, ::state::ColorMask);
	fn update_buffer(&mut self, b::Buffer, Box<::Blob + Send>);
	fn update_texture(&mut self, ::tex::TextureKind, b::Texture,
					  ::tex::ImageInfo, Box<::Blob + Send>);
	fn call_clear(&mut self, t::ClearData);
	fn call_draw(&mut self, ::PrimitiveType, ::VertexCount, ::VertexCount);
	fn call_draw_indexed(&mut self, ::PrimitiveType, ::IndexType, ::IndexCount,
						 ::IndexCount);
}
