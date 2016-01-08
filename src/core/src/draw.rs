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
use {MAX_COLOR_TARGETS};
use {Resources, IndexType, InstanceCount, VertexCount};
use {pso, shade};
use state as s;

type Offset = u32;
type Size = u32;

/// A universal clear color supporting integet formats
/// as well as the standard floating-point.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub enum ClearColor {
    /// Standard floating-point vec4 color
    Float([f32; 4]),
    /// Integer vector to clear ivec4 targets.
    Int([i32; 4]),
    /// Unsigned int vector to clear uvec4 targets.
    Uint([u32; 4]),
}

/// Complete clear data for a given pixel target set.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct ClearSet(
    pub [Option<ClearColor>; MAX_COLOR_TARGETS],
    pub Option<target::Depth>,
    pub Option<target::Stencil>
);

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
    /// Reset the command buffer contents, retain the allocated storage
    fn reset(&mut self);
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
    /// Clear render targets
    fn clear(&mut self, ClearSet);
    /// Draw a primitive
    fn call_draw(&mut self, VertexCount, VertexCount, InstanceOption);
    /// Draw a primitive with index buffer
    fn call_draw_indexed(&mut self, IndexType,
                         VertexCount, VertexCount,
                         VertexCount, InstanceOption);
}

macro_rules! impl_clear {
    { $( $ty:ty = $sub:ident[$a:expr, $b:expr, $c:expr, $d:expr], )* } => {
        $(
            impl From<$ty> for ClearColor {
                fn from(v: $ty) -> ClearColor {
                    ClearColor::$sub([v[$a], v[$b], v[$c], v[$d]])
                }
            }
        )*
    }
}

impl_clear! {
    [f32; 4] = Float[0, 1, 2, 3],
    [f32; 3] = Float[0, 1, 2, 0],
    [f32; 2] = Float[0, 1, 0, 0],
    [i32; 4] = Int  [0, 1, 2, 3],
    [i32; 3] = Int  [0, 1, 2, 0],
    [i32; 2] = Int  [0, 1, 0, 0],
    [u32; 4] = Uint [0, 1, 2, 3],
    [u32; 3] = Uint [0, 1, 2, 0],
    [u32; 2] = Uint [0, 1, 0, 0],
}

impl From<f32> for ClearColor {
    fn from(v: f32) -> ClearColor {
        ClearColor::Float([v, 0.0, 0.0, 0.0])
    }
}
impl From<i32> for ClearColor {
    fn from(v: i32) -> ClearColor {
        ClearColor::Int([v, 0, 0, 0])
    }
}
impl From<u32> for ClearColor {
    fn from(v: u32) -> ClearColor {
        ClearColor::Uint([v, 0, 0, 0])
    }
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
