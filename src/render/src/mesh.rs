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

//! Mesh loading.
//!
//! A `Mesh` describes the geometry of an object. All or part of a `Mesh` can be drawn with a
//! single draw call. A `Mesh` consists of a series of vertices, each with a certain amount of
//! user-specified attributes. These attributes are fed into shader programs. The easiest way to
//! create a mesh is to use the `#[vertex_format]` attribute on a struct, upload them into a
//! `Buffer`, and then use `Mesh::from`.

use gfx_core::handle;
use gfx_core::{Primitive, Resources, VertexCount};
use gfx_core::draw::InstanceOption;
use gfx_core::factory::{BufferRole, Factory};

/// Description of a subset of `Mesh` data to render.
///
/// Only vertices between `start` and `end` are rendered. The
/// source of the vertex data is dependent on the `kind` value.
///
/// The `primitive` defines how the mesh contents are interpreted.
/// For example,  `Point` typed vertex slice can be used to do shape
/// blending, while still rendereing it as an indexed `TriangleList`.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice<R: Resources> {
    /// Start index of vertices to draw.
    pub start: VertexCount,
    /// End index of vertices to draw.
    pub end: VertexCount,
    /// Instancing configuration.
    pub instances: InstanceOption,
    /// Source of the vertex ordering when drawing.
    pub kind: SliceKind<R>,
}

impl<R: Resources> Slice<R> {
    /// Get the number of primitives in this slice.
    pub fn get_prim_count(&self, prim: Primitive) -> u32 {
        use gfx_core::Primitive::*;
        let nv = (self.end - self.start) as u32;
        match prim {
            Point => nv,
            Line => nv / 2,
            LineStrip => (nv-1),
            TriangleList => nv / 3,
            TriangleStrip | TriangleFan => (nv-2) / 3,
        }
    }
}

/// Source of vertex ordering for a slice
#[derive(Clone, Debug, PartialEq)]
pub enum SliceKind<R: Resources> {
    /// Render vertex data directly from the `Mesh`'s buffer.
    Vertex,
    /// The `Index*` buffer contains a list of indices into the `Mesh`
    /// data, so every vertex attribute does not need to be duplicated, only
    /// its position in the `Mesh`. The base index is added to this index
    /// before fetching the vertex from the buffer.  For example, when drawing
    /// a square, two triangles are needed.  Using only `Vertex`, one
    /// would need 6 separate vertices, 3 for each triangle. However, two of
    /// the vertices will be identical, wasting space for the duplicated
    /// attributes.  Instead, the `Mesh` can store 4 vertices and an
    /// `Index8` can be used instead.
    Index8(handle::Buffer<R, u8>, VertexCount),
    /// As `Index8` but with `u16` indices
    Index16(handle::Buffer<R, u16>, VertexCount),
    /// As `Index8` but with `u32` indices
    Index32(handle::Buffer<R, u32>, VertexCount),
}

/// A helper trait to build index slices from data.
pub trait ToIndexSlice<R: Resources> { //TODO: remove/refactor it
    /// Make an index slice.
    fn to_slice<F: Factory<R>>(self, factory: &mut F) -> Slice<R>;
}

macro_rules! impl_slice {
    ($ty:ty, $index:ident) => (
        impl<R: Resources> From<handle::Buffer<R, $ty>> for Slice<R> {
            fn from(buf: handle::Buffer<R, $ty>) -> Slice<R> {
                Slice {
                    start: 0,
                    end: buf.len() as VertexCount,
                    instances: None,
                    kind: SliceKind::$index(buf, 0)
                }
            }
        }
        impl<'a, R: Resources> ToIndexSlice<R> for &'a [$ty] {
            fn to_slice<F: Factory<R>>(self, factory: &mut F) -> Slice<R> {
                //debug_assert!(self.len() <= factory.get_capabilities().max_index_count);
                factory.create_buffer_static(self, BufferRole::Index).into()
            }
        }
    )
}

impl_slice!(u8, Index8);
impl_slice!(u16, Index16);
impl_slice!(u32, Index32);
