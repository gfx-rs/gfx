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

use device as d;
use device::attrib as a;

/// Describes a single attribute of a vertex buffer, including its type, name, etc.
#[deriving(Clone, PartialEq, Show)]
pub struct Attribute {
    /// A name to match the shader input
    pub name: String,
    /// Vertex buffer to contain the data
    pub buffer: d::RawBufferHandle,
    /// Format of the attribute
    pub format: a::Format,
}

/// A trait implemented automatically for user vertex structure by
/// `#[vertex_format] attribute
pub trait VertexFormat {
    /// Create the attributes for this type, using the given buffer.
    fn generate(Option<Self>, buffer: d::RawBufferHandle) -> Vec<Attribute>;
}

/// Describes geometry to render.
#[deriving(Clone, PartialEq, Show)]
pub struct Mesh {
    /// Number of vertices in the mesh.
    pub num_vertices: d::VertexCount,
    /// Vertex attributes to use.
    pub attributes: Vec<Attribute>,
}

impl Mesh {
    /// Create a new mesh, which is a `TriangleList` with no attributes and `nv` vertices.
    pub fn new(nv: d::VertexCount) -> Mesh {
        Mesh {
            num_vertices: nv,
            attributes: Vec::new(),
        }
    }

    /// Create a new `Mesh` from a struct that implements `VertexFormat` and a buffer.
    pub fn from_format<V: VertexFormat>(buf: d::BufferHandle<V>, nv: d::VertexCount) -> Mesh {
        Mesh {
            num_vertices: nv,
            attributes: VertexFormat::generate(None::<V>, buf.raw()),
        }
    }

    /// Create a new intanced `Mesh` given a vertex buffer and an instance buffer.
    pub fn from_format_instanced<V: VertexFormat, U: VertexFormat>(
                                 buf: d::BufferHandle<V>, nv: d::VertexCount,
                                 inst: d::BufferHandle<U>) -> Mesh {
        let per_vertex   = VertexFormat::generate(None::<V>, buf.raw());
        let per_instance = VertexFormat::generate(None::<U>, inst.raw());

        let mut attributes = per_vertex;
        for mut at in per_instance.move_iter() {
            at.format.instance_rate = 1;
            attributes.push(at);
        }

        Mesh {
            num_vertices: nv,
            attributes: attributes,
        }
    }

    /// Return a vertex slice of the whole mesh
    pub fn get_slice(&self, pt: d::PrimitiveType) -> Slice {
        VertexSlice(pt, 0, self.num_vertices)
    }
}

/// Description of a subset of `Mesh` data to render.
/// We provide a primitive type in a slice because it is how we interpret mesh
/// contents. For example, we can have a `Point` typed vertex slice to do shape
/// blending, while still rendereing it as an indexed `TriangleList`.
#[deriving(Clone, Show)]
pub enum Slice  {
    /// Render vertex data directly from the `Mesh`'s buffer, using only the vertices between the two
    /// endpoints.
    VertexSlice(d::PrimitiveType, d::VertexCount, d::VertexCount),
    /// The `IndexSlice*` buffer contains a list of indices into the `Mesh` data, so every vertex
    /// attribute does not need to be duplicated, only its position in the `Mesh`.  For example,
    /// when drawing a square, two triangles are needed.  Using only `VertexSlice`, one would need
    /// 6 separate vertices, 3 for each triangle. However, two of the vertices will be identical,
    /// wasting space for the duplicated attributes.  Instead, the `Mesh` can store 4 vertices and
    /// an `IndexSlice8` can be used instead.
    IndexSlice8(d::PrimitiveType, d::BufferHandle<u8>, d::IndexCount, d::IndexCount),
    /// As `IndexSlice8` but with `u16` indices
    IndexSlice16(d::PrimitiveType, d::BufferHandle<u16>, d::IndexCount, d::IndexCount),
    /// As `IndexSlice8` but with `u32` indices
    IndexSlice32(d::PrimitiveType, d::BufferHandle<u32>, d::IndexCount, d::IndexCount),
}

/// Describes kinds of errors that may occur in the mesh linking
#[deriving(Clone, Show)]
pub enum LinkError {
    /// An attribute index is out of supported bounds
    ErrorMeshAttribute(uint),
    /// An input index is out of supported bounds
    ErrorShaderInput(uint),
}

static BITS_PER_ATTRIBUTE: uint = 4;
static MAX_SHADER_INPUTS: uint = 64 / BITS_PER_ATTRIBUTE;
static MESH_ATTRIBUTE_MASK: uint = (1u << BITS_PER_ATTRIBUTE) - 1;

/// Iterates over mesh attributes in a specific order
pub struct AttributeIterator {
    value: u64,
}

impl Iterator<uint> for AttributeIterator {
    fn next(&mut self) -> Option<uint> {
        let id = (self.value as uint) & MESH_ATTRIBUTE_MASK;
        self.value >>= BITS_PER_ATTRIBUTE;
        Some(id)
    }
}

/// The strcture holding remapping table from shader inputs to mesh attributes
pub struct Link {
    table: u64,
}

impl Link {
    /// Construct a new link from an iterator over attribute indices
    pub fn from_iter<I: Iterator<uint>>(iter: I) -> Result<Link, LinkError> {
        let mut table = 0u64;
        for (input, attrib) in iter.enumerate() {
            if input >= MAX_SHADER_INPUTS {
                return Err(ErrorShaderInput(input))
            }else if attrib > MESH_ATTRIBUTE_MASK {
                return Err(ErrorMeshAttribute(attrib))
            }else {
                table |= attrib as u64 << (input * BITS_PER_ATTRIBUTE);
            }
        }
        Ok(Link {
            table: table,
        })
    }

    /// Convert to an iterator returning attribute indices
    pub fn to_iter(&self) -> AttributeIterator {
        AttributeIterator {
            value: self.table,
        }
    }
}
