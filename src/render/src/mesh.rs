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

//! Slices
//!
//! See `Slice`-structure documentation for more information on this module.

use gfx_core::handle;
use gfx_core::{Primitive, Resources, VertexCount};
use gfx_core::draw::InstanceOption;
use gfx_core::factory::{Bind, BufferRole, Factory};

/// A `Slice` dictates in which and in what order vertices get processed. It is required for
/// processing a PSO.
///
/// # Overview
/// A `Slice` object in essence dictates in what order the vertices in a `VertexBuffer` get
/// processed. To do this, it contains an internal index-buffer. This `Buffer` is a list of
/// indices into this `VertexBuffer` (vertex-index). A vertex-index of 0 represents the first
/// vertex in the `VertexBuffer`, a vertex-index of 1 represents the second, 2 represents the
/// third, and so on. The vertex-indices in the index-buffer are read in order; every vertex-index
/// tells the pipeline which vertex to process next. 
///
/// Because the same index can re-appear multiple times, duplicate-vertices can be avoided. For
/// instance, if you want to draw a square, you need two triangles, and thus six vertices. Because
/// the same index can reappear multiple times, this means we can instead use 4 vertices, and 6
/// vertex-indices.
///
/// This index-buffer has a few variants. See the `SliceKind` documentation for a detailed
/// description.
///
/// The `start` and `end` properties say where in the index-buffer to start and stop reading.
/// Setting `start` to 0, and `end` to the length of the index-buffer, will cause the entire
/// index-buffer to be processed. 
///
/// # Constuction & Handling
/// The `Slice` structure gets constructed automatically when creating a `VertexBuffer` using a
/// `Factory`.
///
/// A `Slice` buffer is required to process a PSO, as it contains the needed information on in what
/// order to draw which vertices. As such, every `draw` call on an `Encoder` requires a `Slice`.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice<R: Resources> {
    /// The start index of the index-buffer. Processing will start at this location in the
    /// index-buffer. 
    pub start: VertexCount,
    /// The end index in the index-buffer. Processing will stop at this location (exclusive) in
    /// the index buffer.
    pub end: VertexCount,
    /// Instancing configuration.
    pub instances: InstanceOption,
    /// Represents the type of index-buffer used. 
    pub kind: SliceKind<R>,
}

impl<R: Resources> Slice<R> {
    /// Calculates the number of primitives of the specified type in this `Slice`.
    pub fn get_prim_count(&self, prim: Primitive) -> u32 {
        use gfx_core::Primitive::*;
        let nv = (self.end - self.start) as u32;
        match prim {
            PointList => nv,
            LineList => nv / 2,
            LineStrip => (nv-1),
            TriangleList => nv / 3,
            TriangleStrip => (nv-2) / 3,
        }
    }
}

/// Type of index-buffer used in a Slice.
///
/// The `Vertex` represents a hypothetical index-buffer from 0 to infinity. In other words, all 
/// vertices get processed in order. Do note that the `Slice`' `start` and `end` restrictions still
/// apply for this variant. To render every vertex in the `VertexBuffer`, you would set `start` to
/// 0, and `end` to the `VertexBuffer`'s length.
///
/// The `Index*` variants represent an actual `Buffer` with a list of vertex-indices. The numeric 
/// suffix specifies the amount of bits to use per index. Each of these also contains a
/// base-vertex. This is the index of the first vertex in the `VertexBuffer`. This value will be
/// added to every index in the index-buffer, effectively moving the start of the `VertexBuffer` to
/// this base-vertex.
#[derive(Clone, Debug, PartialEq)]
pub enum SliceKind<R: Resources> {
    /// Represents a hypothetical index-buffer from 0 to infinity. In other words, all vertices
    /// get processed in order.
    Vertex,
    /// An index-buffer with unsigned 8 bit indices. 
    Index8(handle::Buffer<R, u8>, VertexCount),
    /// An index-buffer with unsigned 16 bit indices.
    Index16(handle::Buffer<R, u16>, VertexCount),
    /// An index-buffer with unsigned 32 bit indices.
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
                factory.create_buffer_const(self, BufferRole::Index, Bind::empty())
                       .unwrap().into()
            }
        }
    )
}

impl_slice!(u8, Index8);
impl_slice!(u16, Index16);
impl_slice!(u32, Index32);
