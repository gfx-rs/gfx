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

use draw_state::target;
use {Resources, IndexType, InstanceCount, VertexCount};
use {pso, shade, tex};
use state as s;

type Offset = u32;
type Size = u32;

/// The place of some data in the data buffer.
#[derive(Clone, Copy, PartialEq, Debug)]
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
    #[cfg(unstable)]
    #[inline(always)]
    pub fn add_struct<T: Copy>(&mut self, v: &T) -> DataPointer {
        use std::slice::ref_slice;
        self.add_vec(ref_slice(v))
    }

    /// Copy a given structure into the buffer, return the offset and the size.
    #[cfg(not(unstable))]
    pub fn add_struct<T: Copy>(&mut self, v: &T) -> DataPointer {
        use std::{ptr, mem};
        let offset = self.buf.len();
        let size = mem::size_of::<T>();
        self.buf.reserve(size);
        unsafe {
            self.buf.set_len(offset + size);
            ptr::copy((v as *const T) as *const u8,
                             &mut self.buf[offset] as *mut u8,
                             size);
        };
        DataPointer(offset as Offset, size as Size)
    }

    /// Copy a given vector slice into the buffer
    pub fn add_vec<T: Copy>(&mut self, v: &[T]) -> DataPointer {
        use std::{ptr, mem};
        let offset = self.buf.len();
        let size = mem::size_of::<T>() * v.len();
        self.buf.reserve(size);
        unsafe {
            self.buf.set_len(offset + size);
            ptr::copy(v.as_ptr() as *const u8,
                             &mut self.buf[offset] as *mut u8,
                             size);
        }
        DataPointer(offset as Offset, size as Size)
    }

    /// Return a reference to a stored data object.
    pub fn get_ref(&self, data: DataPointer) -> &[u8] {
        let DataPointer(offset, size) = data;
        &self.buf[offset as usize ..offset as usize + size as usize]
    }
}

/// Optional instance parameters
pub type InstanceOption = Option<(InstanceCount, VertexCount)>;

/// An interface of the abstract command buffer. It collects commands in an
/// efficient API-specific manner, to be ready for execution on the device.
#[allow(missing_docs)]
pub trait CommandBuffer<R: Resources> {
    /// Clone as an empty buffer
    fn clone_empty(&self) -> Self;
    /// Clear the command buffer contents, retain the allocated storage
    fn clear(&mut self);
    /// Bind a pipeline state object
    fn bind_pipeline_state(&mut self, R::PipelineStateObject);
    /// Bind a complete set of vertex buffers
    fn bind_vertex_buffers(&mut self, pso::VertexBufferSet<R>);
    /// Bind a complete set of constant buffers
    fn bind_constant_buffers(&mut self, pso::ConstantBufferSet<R>);
    /// Bind a global constant
    fn bind_global_constant(&mut self, shade::Location, shade::UniformValue);
    /// Bind a complete set of shader resource views
    fn bind_resource_views(&mut self, pso::ResourceViewSet<R>);
    /// Bind a complete set of unordered access views
    fn bind_unordered_views(&mut self, pso::UnorderedViewSet<R>);
    /// Bind a complete set of samplers
    fn bind_samplers(&mut self, pso::SamplerSet<R>);
    /// Bind a complete set of pixel targets, including multiple
    /// colors views and an optional depth/stencil view.
    fn bind_pixel_targets(&mut self, pso::PixelTargetSet<R>);
    /// Bind an index buffer
    fn bind_index(&mut self, R::Buffer);
    /// Set scissor test
    fn set_scissor(&mut self, Option<target::Rect>);
    /// Set reference values for the blending and stencil front/back
    fn set_ref_values(&mut self, s::RefValues);
    /// Update a vertex/index/uniform buffer
    fn update_buffer(&mut self, R::Buffer, DataPointer, usize);
    /// Update a texture region
    fn update_texture(&mut self, tex::Kind, R::Texture,
                      tex::ImageInfo, DataPointer);
    /// Clear target surfaces
    fn call_clear(&mut self, target::ClearData, target::Mask);
    /// Draw a primitive
    fn call_draw(&mut self, VertexCount, VertexCount, InstanceOption);
    /// Draw a primitive with index buffer
    fn call_draw_indexed(&mut self, IndexType,
                         VertexCount, VertexCount,
                         VertexCount, InstanceOption);
}

/// Type of the gamma transformation for framebuffer writes.
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Gamma {
    /// Process in linear color space.
    Original,
    /// Convert to sRGB color space.
    Convert,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_data_buffer() {
        let mut buf = super::DataBuffer::new();
        assert_eq!(buf.add_struct(&(0u8, false)), super::DataPointer(0, 2));
        assert_eq!(buf.add_vec(&[5i32, 6i32]), super::DataPointer(2, 8));
    }
}
