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

type Offset = u32;
type Size = u32;

/// The place of some data in the data buffer.
#[deriving(PartialEq, Show)]
pub struct DataPointer(Offset, Size);

/// A buffer of data accompanying the commands. It can be vertex data, texture
/// updates, uniform blocks, or even some draw states.
pub struct DataBuffer {
    buf: Vec<u8>,
}

impl DataBuffer {
    /// Create a fresh new data buffer.
    pub fn new() -> DataBuffer {
        DataBuffer {
            buf: Vec::new(),
        }
    }

    /// Clear all the data but retain the allocated storage.
    pub fn clear(&mut self) {
        unsafe { self.buf.set_len(0); }
    }

    /// Copy a given structure into the buffer, return the offset and the size.
    #[inline(always)]
    pub fn add_struct<T: Copy>(&mut self, v: &T) -> DataPointer {
        use std::slice::ref_slice;
        self.add_vec(ref_slice(v))
    }

    /// Copy a given vector slice into the buffer
    pub fn add_vec<T: Copy>(&mut self, v: &[T]) -> DataPointer {
        use std::{mem, slice};
        let offset = self.buf.len();
        let size = mem::size_of::<T>() * v.len();
        self.buf.reserve_additional(size);
        unsafe {
            self.buf.set_len(offset + size);
            slice::raw::buf_as_slice(v.as_ptr() as *const u8, size,
                |slice|
                slice::bytes::copy_memory(self.buf.slice_from_mut(offset), slice)
            );
        }
        DataPointer(offset as Offset, size as Size)
    }

    /// Return a reference to a stored data object.
    pub fn get_ref(&self, data: DataPointer) -> &[u8] {
        let DataPointer(offset, size) = data;
        self.buf.slice(offset as uint, offset as uint + size as uint)
    }
}

#[allow(missing_docs)]    //TODO
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
    fn update_buffer(&mut self, back::Buffer, DataPointer, uint);
    fn update_texture(&mut self, tex::TextureKind, back::Texture,
                      tex::ImageInfo, DataPointer);
    fn call_clear(&mut self, target::ClearData, target::Mask);
    fn call_draw(&mut self, ::PrimitiveType, ::VertexCount, ::VertexCount,
                 Option<(::InstanceCount, ::VertexCount)>);
    fn call_draw_indexed(&mut self, ::PrimitiveType, ::IndexType, ::VertexCount,
                         ::VertexCount, ::VertexCount, Option<(::InstanceCount, ::VertexCount)>);
    fn call_blit(&mut self, target::Rect, target::Rect, target::Mask);
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_data_buffer() {
        let mut buf = super::DataBuffer::new();
        assert_eq!(buf.add_struct(&(0u, false)), super::DataPointer(0, 16));
        assert_eq!(buf.add_vec(&[5i, 6i]), super::DataPointer(16, 16));
    }
}
