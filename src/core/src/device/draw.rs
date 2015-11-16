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
use device as d;

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
pub type InstanceOption = Option<(d::InstanceCount, d::VertexCount)>;

/// An interface of the abstract command buffer. It collects commands in an
/// efficient API-specific manner, to be ready for execution on the device.
#[allow(missing_docs)]
pub trait CommandBuffer<R: d::Resources> {
    /// An empty constructor
    fn new() -> Self;
    /// Clear the command buffer contents, retain the allocated storage
    fn clear(&mut self);
    /// Bind a shader program
    fn bind_program(&mut self, R::Program);
    /// Bind a pipeline state object
    fn bind_pipeline_state(&mut self, R::PipelineState);
    /// Bind a complete set of vertex buffers
    fn bind_vertex_buffers(&mut self, d::pso::VertexBufferSet<R>);
    /// Bind an array buffer object
    fn bind_array_buffer(&mut self, R::ArrayBuffer);
    /// Bind a vertex attribute
    fn bind_attribute(&mut self, d::AttributeSlot, R::Buffer, d::attrib::Format);
    /// Bind an index buffer
    fn bind_index(&mut self, R::Buffer);
    /// Bind a frame buffer object
    fn bind_frame_buffer(&mut self, Access, R::FrameBuffer, Gamma);
    /// Unbind any surface from the specified target slot
    fn unbind_target(&mut self, Access, Target);
    /// Bind a surface to the specified target slot
    fn bind_target_surface(&mut self, Access, Target, R::Surface);
    /// Bind a level of the texture to the specified target slot
    fn bind_target_texture(&mut self, Access, Target, R::Texture,
                           target::Level, Option<target::Layer>);
    /// Bind a uniform block
    fn bind_uniform_block(&mut self, R::Program,
                          d::UniformBufferSlot, d::UniformBlockIndex,
                          R::Buffer);
    /// Bind a single uniform in the default block
    fn bind_uniform(&mut self, d::shade::Location, d::shade::UniformValue);
    /// Bind a texture
    fn bind_texture(&mut self, d::TextureSlot, d::tex::Kind,
                    R::Texture, Option<(R::Sampler, d::tex::SamplerInfo)>);
    /// Select, which color buffers are going to be targetted by the shader
    fn set_draw_color_buffers(&mut self, d::ColorSlot);
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
                         Option<::state::Stencil>, ::state::CullFace);
    /// Set blend state
    fn set_blend(&mut self, d::ColorSlot, Option<::state::Blend>);
    /// Set reference values for the blending and stencil front/back.
    fn set_ref_values(&mut self, target::ColorValue,
                      target::Stencil, target::Stencil);
    /// Update a vertex/index/uniform buffer
    fn update_buffer(&mut self, R::Buffer, DataPointer, usize);
    /// Update a texture region
    fn update_texture(&mut self, d::tex::Kind, R::Texture,
                      d::tex::ImageInfo, DataPointer);
    /// Clear target surfaces
    fn call_clear(&mut self, target::ClearData, target::Mask);
    /// Draw a primitive
    fn call_draw(&mut self, d::PrimitiveType, d::VertexCount,
                 d::VertexCount, InstanceOption);
    /// Draw a primitive with index buffer
    fn call_draw_indexed(&mut self, d::PrimitiveType, d::IndexType,
                         d::VertexCount, d::VertexCount,
                         d::VertexCount, InstanceOption);
    /// Blit from one target to another
    fn call_blit(&mut self, target::Rect, target::Rect, target::Mirror, target::Mask);
}

/// Type of the frame buffer access.
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Access {
    /// Draw access
    Draw,
    /// Read access
    Read,
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

/// When rendering, each "output" of the fragment shader goes to a specific target. A `Plane` can
/// be bound to a target, causing writes to that target to affect the `Plane`.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Target {
    /// Color data.
    ///
    /// # Portability Note
    ///
    /// The device is only required to expose one color target.
    Color(u8),
    /// Depth data.
    Depth,
    /// Stencil data.
    Stencil,
    /// A target for both depth and stencil data at once.
    DepthStencil,
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
