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

use core::{handle, buffer};
use core::{Primitive, Resources, VertexCount};
use core::command::InstanceParams;
use core::factory::Factory;
use core::memory::Bind;
use format::Format;
use pso;

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
/// This index-buffer has a few variants. See the `IndexBuffer` documentation for a detailed
/// description.
///
/// The `start` and `end` fields say where in the index-buffer to start and stop reading.
/// Setting `start` to 0, and `end` to the length of the index-buffer, will cause the entire
/// index-buffer to be processed. The `base_vertex` dictates the index of the first vertex
/// in the `VertexBuffer`. This essentially moves the the start of the `VertexBuffer`, to the
/// vertex with this index.
///
/// # Constuction & Handling
/// The `Slice` structure can be constructed automatically when using a `Factory` to create a
/// vertex buffer. If needed, it can also be created manually.
///
/// A `Slice` is required to process a PSO, as it contains the needed information on in what order
/// to draw which vertices. As such, every `draw` call on an `Encoder` requires a `Slice`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Slice<R: Resources> {
    /// The start index of the index-buffer. Processing will start at this location in the
    /// index-buffer. 
    pub start: VertexCount,
    /// The end index in the index-buffer. Processing will stop at this location (exclusive) in
    /// the index buffer.
    pub end: VertexCount,
    /// This is the index of the first vertex in the `VertexBuffer`. This value will be added to
    /// every index in the index-buffer, effectively moving the start of the `VertexBuffer` to this
    /// base-vertex.
    pub base_vertex: VertexCount,
    /// Instancing configuration.
    pub instances: Option<InstanceParams>,
    /// Represents the type of index-buffer used. 
    pub buffer: IndexBuffer<R>,
}

impl<R: Resources> Slice<R> {
    /// Creates a new `Slice` to match the supplied vertex buffer, from start to end, in order.
    pub fn new_match_vertex_buffer<V>(vbuf: &handle::Buffer<R, V>) -> Self
                                      where V: pso::buffer::Structure<Format> {
        Slice {
            start: 0,
            end: vbuf.len() as u32,
            base_vertex: 0,
            instances: None,
            buffer: IndexBuffer::Auto,
        }
    }
    
    /// Calculates the number of primitives of the specified type in this `Slice`.
    pub fn get_prim_count(&self, prim: Primitive) -> u32 {
        use core::Primitive as p;
        let nv = (self.end - self.start) as u32;
        match prim {
            p::PointList => nv,
            p::LineList => nv / 2,
            p::LineStrip => (nv-1),
            p::TriangleList => nv / 3,
            p::TriangleStrip => (nv-2) / 3,
            p::LineListAdjacency => nv / 4,
            p::LineStripAdjacency => (nv-3),
            p::TriangleListAdjacency => nv / 6,
            p::TriangleStripAdjacency => (nv-4) / 2,
            p::PatchList(num) => nv / (num as u32),
        }
    }

    /// Divides one slice into two at an index.
    ///
    /// The first will contain the range in the index-buffer [self.start, mid) (excluding the index mid itself) and the
    /// second will contain the range [mid, self.end).
    pub fn split_at(&self, mid: VertexCount) -> (Self, Self) {
        let mut first = self.clone();
        let mut second = self.clone();
        first.end = mid;
        second.start = mid;

        (first, second)
    }
}

/// Type of index-buffer used in a Slice.
///
/// The `Auto` variant represents a hypothetical index-buffer from 0 to infinity. In other words,
/// all vertices get processed in order. Do note that the `Slice`' `start` and `end` restrictions
/// still apply for this variant. To render every vertex in the `VertexBuffer`, you would set
/// `start` to 0, and `end` to the `VertexBuffer`'s length.
///
/// The `Index*` variants represent an actual `Buffer` with a list of vertex-indices. The numeric 
/// suffix specifies the amount of bits to use per index. Each of these also contains a
/// base-vertex. This is the index of the first vertex in the `VertexBuffer`. This value will be
/// added to every index in the index-buffer, effectively moving the start of the `VertexBuffer` to
/// this base-vertex.
///
/// # Construction & Handling
/// A `IndexBuffer` can be constructed using the `IntoIndexBuffer` trait, from either a slice or a
/// `Buffer` of integers, using a factory.
///
/// An `IndexBuffer` is exclusively used to create `Slice`s.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum IndexBuffer<R: Resources> {
    /// Represents a hypothetical index-buffer from 0 to infinity. In other words, all vertices
    /// get processed in order.
    Auto,
    /// An index-buffer with unsigned 16 bit indices.
    Index16(handle::Buffer<R, u16>),
    /// An index-buffer with unsigned 32 bit indices.
    Index32(handle::Buffer<R, u32>),
}

impl<R: Resources> Default for IndexBuffer<R> {
    fn default() -> Self {
        IndexBuffer::Auto
    }
}
/// A helper trait to create `IndexBuffers` from different kinds of data.
pub trait IntoIndexBuffer<R: Resources> {
    /// Turns self into an `IndexBuffer`.
    fn into_index_buffer<F: Factory<R> + ?Sized>(self, factory: &mut F) -> IndexBuffer<R>;
}

impl<R: Resources> IntoIndexBuffer<R> for IndexBuffer<R> {
    fn into_index_buffer<F: Factory<R> + ?Sized>(self, _: &mut F) -> IndexBuffer<R> {
        self
    }
}

impl<R: Resources> IntoIndexBuffer<R> for () {
    fn into_index_buffer<F: Factory<R> + ?Sized>(self, _: &mut F) -> IndexBuffer<R> {
        IndexBuffer::Auto
    }
}

macro_rules! impl_index_buffer {
    ($prim_ty:ty, $buf_ty:ident) => (
        impl<R: Resources> IntoIndexBuffer<R> for handle::Buffer<R, $prim_ty> {
            fn into_index_buffer<F: Factory<R> + ?Sized>(self, _: &mut F) -> IndexBuffer<R> {
                IndexBuffer::$buf_ty(self)
            }
        }
        
        impl<'s, R: Resources> IntoIndexBuffer<R> for &'s [$prim_ty] {
            fn into_index_buffer<F: Factory<R> + ?Sized>(self, factory: &mut F) -> IndexBuffer<R> {
                factory.create_buffer_immutable(self, buffer::Role::Index, Bind::empty())
                       .unwrap()
                       .into_index_buffer(factory)
            }
        }
    )
}

impl_index_buffer!(u16, Index16);
impl_index_buffer!(u32, Index32);
