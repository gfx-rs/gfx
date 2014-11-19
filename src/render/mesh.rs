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

use device;
use device::{PrimitiveType, BufferHandle, VertexCount};
use device::attrib;

/// Describes a single attribute of a vertex buffer, including its type, name, etc.
#[deriving(Clone, PartialEq, Show)]
pub struct Attribute {
    /// A name to match the shader input
    pub name: String,
    /// Vertex buffer to contain the data
    pub buffer: device::RawBufferHandle,
    /// Format of the attribute
    pub format: attrib::Format,
}

/// A trait implemented automatically for user vertex structure by
/// `#[vertex_format] attribute
pub trait VertexFormat {
    /// Create the attributes for this type, using the given buffer.
    fn generate(Option<Self>, buffer: device::RawBufferHandle) -> Vec<Attribute>;
}

/// Describes geometry to render.
#[deriving(Clone, PartialEq, Show)]
pub struct Mesh {
    /// Number of vertices in the mesh.
    pub num_vertices: device::VertexCount,
    /// Vertex attributes to use.
    pub attributes: Vec<Attribute>,
}

impl Mesh {
    /// Create a new mesh, which is a `TriangleList` with no attributes and `nv` vertices.
    pub fn new(nv: device::VertexCount) -> Mesh {
        Mesh {
            num_vertices: nv,
            attributes: Vec::new(),
        }
    }

    /// Create a new `Mesh` from a struct that implements `VertexFormat` and a buffer.
    pub fn from_format<V: VertexFormat>(buf: device::BufferHandle<V>, nv: device::VertexCount) -> Mesh {
        Mesh {
            num_vertices: nv,
            attributes: VertexFormat::generate(None::<V>, buf.raw()),
        }
    }

    /// Create a new intanced `Mesh` given a vertex buffer and an instance buffer.
    pub fn from_format_instanced<V: VertexFormat, U: VertexFormat>(
                                 buf: device::BufferHandle<V>, nv: device::VertexCount,
                                 inst: device::BufferHandle<U>) -> Mesh {
        let per_vertex   = VertexFormat::generate(None::<V>, buf.raw());
        let per_instance = VertexFormat::generate(None::<U>, inst.raw());

        let mut attributes = per_vertex;
        for mut at in per_instance.into_iter() {
            at.format.instance_rate = 1;
            attributes.push(at);
        }

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
#[deriving(Clone, Show)]
pub struct Slice {
    /// Start index of vertices to draw.
    pub start: VertexCount,
    /// End index of vertices to draw.
    pub end: VertexCount,
    /// Primitive type to render collections of vertices as.
    pub prim_type: PrimitiveType,
    /// Source of the vertex ordering when drawing.
    pub kind: SliceKind,
}

/// Source of vertex ordering for a slice
#[deriving(Clone, Show)]
pub enum SliceKind {
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
    Index8(BufferHandle<u8>, VertexCount),
    /// As `Index8` but with `u16` indices
    Index16(BufferHandle<u16>, VertexCount),
    /// As `Index8` but with `u32` indices
    Index32(BufferHandle<u32>, VertexCount),
}

/// Helper methods for cleanly getting the slice of a type.
pub trait ToSlice {
    /// Get the slice of a type.
    fn to_slice(&self, pt: PrimitiveType) -> Slice;
}

impl ToSlice for Mesh {
    /// Return a vertex slice of the whole mesh.
    fn to_slice(&self, ty: PrimitiveType) -> Slice {
        Slice {
            start: 0,
            end: self.num_vertices,
            prim_type: ty,
            kind: SliceKind::Vertex
        }
    }
}

impl ToSlice for BufferHandle<u8> {
    /// Return an index slice of the whole buffer.
    fn to_slice(&self, ty: PrimitiveType) -> Slice {
        Slice {
            start: 0,
            end: self.len() as VertexCount,
            prim_type: ty,
            kind: SliceKind::Index8(*self, 0)
        }
    }
}

impl ToSlice for BufferHandle<u16> {
    /// Return an index slice of the whole buffer.
    fn to_slice(&self, ty: PrimitiveType) -> Slice {
        Slice {
            start: 0,
            end: self.len() as VertexCount,
            prim_type: ty,
            kind: SliceKind::Index16(*self, 0)
        }
    }
}

impl ToSlice for BufferHandle<u32> {
    /// Return an index slice of the whole buffer.
    fn to_slice(&self, ty: PrimitiveType) -> Slice {
        Slice {
            start: 0,
            end: self.len() as VertexCount,
            prim_type: ty,
            kind: SliceKind::Index32(*self, 0)
        }
    }
}

/// Describes kinds of errors that may occur in the mesh linking
#[deriving(Clone, Show)]
pub enum LinkError {
    /// An attribute index is out of supported bounds
    MeshAttribute(uint),
    /// An input index is out of supported bounds
    ShaderInput(uint),
}

const BITS_PER_ATTRIBUTE: uint = 4;
const MAX_SHADER_INPUTS: uint = 64 / BITS_PER_ATTRIBUTE;
const MESH_ATTRIBUTE_MASK: uint = (1u << BITS_PER_ATTRIBUTE) - 1;

/// An iterator over mesh attributes.
pub struct AttributeIndices {
    value: u64,
}

impl Iterator<uint> for AttributeIndices {
    fn next(&mut self) -> Option<uint> {
        let id = (self.value as uint) & MESH_ATTRIBUTE_MASK;
        self.value >>= BITS_PER_ATTRIBUTE;
        Some(id)
    }
}

/// Holds a remapping table from shader inputs to mesh attributes.
pub struct Link {
    table: u64,
}

impl Link {
    /// Construct a new link from an iterator over attribute indices.
    pub fn from_iter<I: Iterator<uint>>(iter: I) -> Result<Link, LinkError> {
        let mut table = 0u64;
        for (input, attrib) in iter.enumerate() {
            if input >= MAX_SHADER_INPUTS {
                return Err(LinkError::ShaderInput(input))
            } else if attrib > MESH_ATTRIBUTE_MASK {
                return Err(LinkError::MeshAttribute(attrib))
            } else {
                table |= attrib as u64 << (input * BITS_PER_ATTRIBUTE);
            }
        }
        Ok(Link {
            table: table,
        })
    }

    /// Convert to an iterator returning attribute indices
    pub fn attribute_indices(&self) -> AttributeIndices {
        AttributeIndices {
            value: self.table,
        }
    }
}
