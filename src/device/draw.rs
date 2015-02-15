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

use std::fmt;

use attrib;
use back;
use shade;
use target;
use tex;

type Offset = u32;
type Size = u32;

/// The place of some data in the data buffer.
#[derive(Copy, PartialEq, Debug)]
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
        self.buf.reserve(size);
        unsafe {
            self.buf.set_len(offset + size);
            slice::bytes::copy_memory(&mut self.buf[offset ..],
                                      slice::from_raw_parts(v.as_ptr() as *const u8, size));
        }
        DataPointer(offset as Offset, size as Size)
    }

    /// Return a reference to a stored data object.
    pub fn get_ref(&self, data: DataPointer) -> &[u8] {
        let DataPointer(offset, size) = data;
        &self.buf[offset as usize ..offset as usize + size as usize]
    }
}

/// An interface of the abstract command buffer. It collects commands in an
/// efficient API-specific manner, to be ready for execution on the device.
pub trait CommandBuffer {
    type Buffer:        Copy + fmt::Debug + PartialEq + Clone;
    type ArrayBuffer:   Copy + fmt::Debug + PartialEq + Clone;
    type Program:       Copy + fmt::Debug + PartialEq + Clone;
    type FrameBuffer:   Copy + fmt::Debug + PartialEq + Clone;
    type Surface:       Copy + fmt::Debug + PartialEq + Clone;
    type Texture:       Copy + fmt::Debug + PartialEq + Clone;
    type Sampler:       Copy + fmt::Debug + PartialEq + Clone;

    /// An empty constructor
    fn new() -> Self;
    /// Clear the command buffer contents, retain the allocated storage
    fn clear(&mut self);
    /// Bind a shader program
    fn bind_program(&mut self, back::Program);
    /// Bind an array buffer object
    fn bind_array_buffer(&mut self, back::ArrayBuffer);
    /// Bind a vertex attribute
    fn bind_attribute(&mut self, ::AttributeSlot, back::Buffer, attrib::Format);
    /// Bind an index buffer
    fn bind_index(&mut self, back::Buffer);
    /// Bind a frame buffer object
    fn bind_frame_buffer(&mut self, target::Access, back::FrameBuffer);
    /// Unbind any surface from the specified target slot
    fn unbind_target(&mut self, target::Access, target::Target);
    /// Bind a surface to the specified target slot
    fn bind_target_surface(&mut self, target::Access, target::Target, back::Surface);
    /// Bind a level of the texture to the specified target slot
    fn bind_target_texture(&mut self, target::Access, target::Target, back::Texture,
                           target::Level, Option<target::Layer>);
    /// Bind a uniform block
    fn bind_uniform_block(&mut self, back::Program, ::UniformBufferSlot,
                          ::UniformBlockIndex, back::Buffer);
    /// Bind a single uniform in the default block
    fn bind_uniform(&mut self, shade::Location, shade::UniformValue);
    /// Bind a texture
    fn bind_texture(&mut self, ::TextureSlot, tex::TextureKind, back::Texture,
                    Option<::SamplerHandle<back::GlDevice>>);
    /// Select, which color buffers are going to be targetted by the shader
    fn set_draw_color_buffers(&mut self, usize);
    /// Set primitive topology
    fn set_primitive(&mut self, ::state::Primitive);
    /// Set viewport rectangle
    fn set_viewport(&mut self, target::Rect);
    /// Set multi-sampling state
    fn set_multi_sample(&mut self, Option<::state::MultiSample>);
    /// Set scissor test
    fn set_scissor(&mut self, Option<target::Rect>);
    /// Set depth and stencil states
    fn set_depth_stencil(&mut self, Option<::state::Depth>,
                         Option<::state::Stencil>, ::state::CullMode);
    /// Set blend state
    fn set_blend(&mut self, Option<::state::Blend>);
    /// Set output color mask for all targets
    fn set_color_mask(&mut self, ::state::ColorMask);
    /// Update a vertex/index/uniform buffer
    fn update_buffer(&mut self, back::Buffer, DataPointer, usize);
    /// Update a texture region
    fn update_texture(&mut self, tex::TextureKind, back::Texture,
                      tex::ImageInfo, DataPointer);
    /// Clear target surfaces
    fn call_clear(&mut self, target::ClearData, target::Mask);
    /// Draw a primitive
    fn call_draw(&mut self, ::PrimitiveType, ::VertexCount, ::VertexCount,
                 Option<(::InstanceCount, ::VertexCount)>);
    /// Draw a primitive with index buffer
    fn call_draw_indexed(&mut self, ::PrimitiveType, ::IndexType, ::VertexCount,
                         ::VertexCount, ::VertexCount, Option<(::InstanceCount, ::VertexCount)>);
    /// Blit from one target to another
    fn call_blit(&mut self, target::Rect, target::Rect, target::Mirror, target::Mask);
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
