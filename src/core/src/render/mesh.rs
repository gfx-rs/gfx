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

use device::{BufferRole, Factory, PrimitiveType, Resources, VertexCount};
use device::{attrib, handle, shade};

/// Describes a single attribute of a vertex buffer, including its type, name, etc.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Attribute<R: Resources> {
    /// A name to match the shader input
    pub name: String,
    /// Vertex buffer to contain the data
    pub buffer: handle::RawBuffer<R>,
    /// Format of the attribute
    pub format: attrib::Format,
}

/// A trait implemented automatically for user vertex structure by
/// `#[vertex_format] attribute
pub trait VertexFormat {
    /// Create the attributes for this type, using the given buffer.
    fn generate<R: Resources>(buffer: &handle::Buffer<R, Self>) -> Vec<Attribute<R>> where Self: Sized;
}

/// Describes geometry to render.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Mesh<R: Resources> {
    /// Number of vertices in the mesh.
    pub num_vertices: VertexCount,
    /// Vertex attributes to use.
    pub attributes: Vec<Attribute<R>>,
}

impl<R: Resources> Mesh<R> {
    /// Create a new mesh, which is a `TriangleList` with no attributes and `nv` vertices.
    pub fn new(nv: VertexCount) -> Mesh<R> {
        Mesh {
            num_vertices: nv,
            attributes: Vec::new(),
        }
    }

    /// Create a new `Mesh` from a struct that implements `VertexFormat` and a buffer.
    pub fn from_format<V: VertexFormat>(buf: handle::Buffer<R, V>, nv: VertexCount)
                       -> Mesh<R> {
        Mesh {
            num_vertices: nv,
            attributes: V::generate(&buf),
        }
    }

    /// Create a new intanced `Mesh` given a vertex buffer and an instance buffer.
    ///
    /// `nv` is the number of vertices of the repeated mesh.
    pub fn from_format_instanced<V: VertexFormat, U: VertexFormat>(
                                 buf: handle::Buffer<R, V>,
                                 nv: VertexCount,
                                 inst: handle::Buffer<R, U>) -> Mesh<R> {
        let per_vertex   = V::generate(&buf);
        let per_instance = U::generate(&inst);

        let mut attributes = per_vertex;
        attributes.extend(per_instance.into_iter().map(|mut a| {
            a.format.instance_rate = 1;
            a
        }));

        Mesh {
            num_vertices: nv,
            attributes: attributes,
        }
    }
}

/// Description of a subset of `Mesh` data to render.
///
/// Only vertices between `start` and `end` are rendered. The
/// source of the vertex data is dependent on the `kind` value.
///
/// The `prim_type` defines how the mesh contents are interpreted.
/// For example,  `Point` typed vertex slice can be used to do shape
/// blending, while still rendereing it as an indexed `TriangleList`.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice<R: Resources> {
    /// Start index of vertices to draw.
    pub start: VertexCount,
    /// End index of vertices to draw.
    pub end: VertexCount,
    /// Primitive type to render collections of vertices as.
    pub prim_type: PrimitiveType,
    /// Source of the vertex ordering when drawing.
    pub kind: SliceKind<R>,
}

impl<R: Resources> Slice<R> {
    /// Get the number of primitives in this slice.
    pub fn get_prim_count(&self) -> u32 {
        use device::PrimitiveType::*;
        let nv = (self.end - self.start) as u32;
        match self.prim_type {
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

/// A helper trait for cleanly getting the slice of a type.
pub trait ToSlice<R: Resources> {
    /// Get the slice of a type.
    fn to_slice(&self, pt: PrimitiveType) -> Slice<R>;
}

/// A helper trait to build index slices from data.
pub trait ToIndexSlice<R: Resources> {
    /// Make an index slice.
    fn to_slice<F: Factory<R>>(self, factory: &mut F, prim: PrimitiveType) -> Slice<R>;
}

impl<R: Resources> ToSlice<R> for Mesh<R> {
    /// Return a vertex slice of the whole mesh.
    fn to_slice(&self, ty: PrimitiveType) -> Slice<R> {
        Slice {
            start: 0,
            end: self.num_vertices,
            prim_type: ty,
            kind: SliceKind::Vertex
        }
    }
}

macro_rules! impl_slice {
    ($ty:ty, $index:ident) => (
        impl<R: Resources> ToSlice<R> for handle::Buffer<R, $ty> {
            fn to_slice(&self, prim: PrimitiveType) -> Slice<R> {
                Slice {
                    start: 0,
                    end: self.len() as VertexCount,
                    prim_type: prim,
                    kind: SliceKind::$index(self.clone(), 0)
                }
            }
        }
        impl<'a, R: Resources> ToIndexSlice<R> for &'a [$ty] {
            fn to_slice<F: Factory<R>>(self, factory: &mut F, prim: PrimitiveType) -> Slice<R> {
                //debug_assert!(self.len() <= factory.get_capabilities().max_index_count);
                factory.create_buffer_static(self, BufferRole::Index).to_slice(prim)
            }
        }
    )
}

impl_slice!(u8, Index8);
impl_slice!(u16, Index16);
impl_slice!(u32, Index32);


/// Index of a vertex attribute inside the mesh
pub type AttributeIndex = usize;

/// Describes kinds of errors that may occur in the mesh linking
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// A required attribute was missing.
    AttributeMissing(String),
    /// An attribute's type from the vertex format differed from the type used in the shader.
    AttributeType(String, shade::BaseType),
    /// An attribute index is out of supported bounds
    AttributeIndex(AttributeIndex),
    /// An input index is out of supported bounds
    ShaderInputIndex(usize),
}

const BITS_PER_ATTRIBUTE: AttributeIndex = 4;
const MESH_ATTRIBUTE_MASK: AttributeIndex = (1 << BITS_PER_ATTRIBUTE) - 1;
const MAX_SHADER_INPUTS: usize = 64 / BITS_PER_ATTRIBUTE;

/// An iterator over mesh attributes.
#[derive(Clone, Copy)]
pub struct AttributeIter {
    value: u64,
}

impl Iterator for AttributeIter {
    type Item = AttributeIndex;

    fn next(&mut self) -> Option<AttributeIndex> {
        let id = (self.value as AttributeIndex) & MESH_ATTRIBUTE_MASK;
        self.value >>= BITS_PER_ATTRIBUTE;
        Some(id)
    }
}

/// Holds a remapping table from shader inputs to mesh attributes.
#[derive(Clone, Copy)]
pub struct Link {
    table: u64,
}

impl Link {
    /// Match mesh attributes against shader inputs, produce a mesh link.
    /// Exposed to public to allow external `Batch` implementations to use it.
    pub fn new<R: Resources>(mesh: &Mesh<R>, pinfo: &shade::ProgramInfo)
                             -> Result<Link, Error> {
        let mut indices = Vec::new();
        for sat in pinfo.attributes.iter() {
            match mesh.attributes.iter().enumerate()
                      .find(|&(_, a)| a.name == sat.name) {
                Some((attrib_id, vat)) => match vat.format.elem_type.is_compatible(sat.base_type) {
                    Ok(_) => indices.push(attrib_id),
                    Err(_) => return Err(Error::AttributeType(sat.name.clone(), sat.base_type)),
                },
                None => return Err(Error::AttributeMissing(sat.name.clone())),
            }
        }
        Link::from_iter(indices.into_iter())
    }

    /// Construct a new link from an iterator over attribute indices.
    pub fn from_iter<I: Iterator<Item = AttributeIndex>>(iter: I)
                     -> Result<Link, Error> {
        let mut table = 0u64;
        for (input, attrib) in iter.enumerate() {
            if input >= MAX_SHADER_INPUTS {
                return Err(Error::ShaderInputIndex(input))
            } else if attrib > MESH_ATTRIBUTE_MASK {
                return Err(Error::AttributeIndex(attrib))
            } else {
                table |= (attrib as u64) << (input * BITS_PER_ATTRIBUTE);
            }
        }
        Ok(Link {
            table: table,
        })
    }

    /// Convert to an iterator returning attribute indices
    pub fn to_iter(&self) -> AttributeIter {
        AttributeIter {
            value: self.table,
        }
    }
}
