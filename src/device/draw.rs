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

//! Command Buffer device interface

use attrib;
use back;
use shade;
use target;
use tex;

use blob::Blob;

#[allow(missing_doc)]    //TODO
pub trait CommandBuffer {
    /// An empty constructor
    fn new() -> Self;
    /// Clear the command buffer contents, retain the allocated storage
    fn clear(&mut self);
    fn bind_program(&mut self, back::Program);
    fn bind_array_buffer(&mut self, back::ArrayBuffer);
    fn bind_attribute(&mut self, ::AttributeSlot, back::Buffer, attrib::Format);
    fn bind_index(&mut self, back::Buffer);
    fn bind_frame_buffer(&mut self, target::Access, back::FrameBuffer);
    /// Unbind any surface from the specified target slot
    fn unbind_target(&mut self, target::Access, target::Target);
    /// Bind a surface to the specified target slot
    fn bind_target_surface(&mut self, target::Access, target::Target, back::Surface);
    /// Bind a level of the texture to the specified target slot
    fn bind_target_texture(&mut self, target::Access, target::Target, back::Texture,
                           target::Level, Option<target::Layer>);
    fn bind_uniform_block(&mut self, back::Program, ::UniformBufferSlot,
                          ::UniformBlockIndex, back::Buffer);
    fn bind_uniform(&mut self, shade::Location, shade::UniformValue);
    fn bind_texture(&mut self, ::TextureSlot, tex::TextureKind, back::Texture,
                    Option<::SamplerHandle>);
    fn set_primitive(&mut self, ::state::Primitive);
    fn set_viewport(&mut self, target::Rect);
    fn set_multi_sample(&mut self, Option<::state::MultiSample>);
    fn set_scissor(&mut self, Option<target::Rect>);
    fn set_depth_stencil(&mut self, Option<::state::Depth>,
                         Option<::state::Stencil>, ::state::CullMode);
    fn set_blend(&mut self, Option<::state::Blend>);
    fn set_color_mask(&mut self, ::state::ColorMask);
    fn update_buffer(&mut self, back::Buffer, Box<Blob<()> + Send>, uint);
    fn update_texture(&mut self, tex::TextureKind, back::Texture,
                      tex::ImageInfo, Box<Blob<()> + Send>);
    fn call_clear(&mut self, target::ClearData, target::Mask);
    fn call_draw(&mut self, ::PrimitiveType, ::VertexCount, ::VertexCount,
                 Option<::InstanceCount>);
    fn call_draw_indexed(&mut self, ::PrimitiveType, ::IndexType, ::IndexCount,
                         ::IndexCount, Option<::InstanceCount>);
}
