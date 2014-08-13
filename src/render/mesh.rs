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

use d = device;
use a = device::attrib;
use device::dev::Buffer;

/// Describes a single attribute of a vertex buffer, including its type, name, etc.
#[deriving(Clone, PartialEq, Show)]
pub struct Attribute {
    /// Vertex buffer to contain the data
    pub buffer: Buffer,
    /// Number of elements per vertex
    pub elem_count: a::Count,
    /// Type of a single element
    pub elem_type: a::Type,
    /// Offset in bytes to the first vertex
    pub offset: a::Offset,
    /// Stride in bytes between consecutive vertices
    pub stride: a::Stride,
    /// A name to match the shader input
    pub name: String,
}

/// A trait implemented automatically for user vertex structure by
/// `#[vertex_format] attribute
pub trait VertexFormat {
    /// Create the attributes for this type, using the given buffer.
    fn generate(Option<Self>, buffer: Buffer) -> Vec<Attribute>;
}

/// Describes geometry to render.
#[deriving(Clone, Show)]
pub struct Mesh {
    /// What primitives to form out of the vertex data.
    pub prim_type: d::PrimitiveType,
    /// Number of vertices in the mesh.
    pub num_vertices: d::VertexCount,
    /// Vertex attributes to use.
    pub attributes: Vec<Attribute>,
}

impl Mesh {
    /// Create a new mesh, which is a `TriangleList` with no attributes and `nv` vertices.
    pub fn new(nv: d::VertexCount) -> Mesh {
        Mesh {
            prim_type: d::TriangleList,
            num_vertices: nv,
            attributes: Vec::new(),
        }
    }

    /// Create a new `Mesh` from a struct that implements `VertexFormat` and a buffer.
    pub fn from<V: VertexFormat>(buf: Buffer, nv: d::VertexCount) -> Mesh {
        Mesh {
            prim_type: d::TriangleList,
            num_vertices: nv,
            attributes: VertexFormat::generate(None::<V>, buf),
        }
    }

    /// Return a vertex slice of the whole mesh
    pub fn get_slice(&self) -> Slice {
        VertexSlice(0, self.num_vertices)
    }
}

/// Description of a subset of `Mesh` data to render.
#[deriving(Clone, Show)]
pub enum Slice  {
    /// Render vertex data directly from the `Mesh`'s buffer, using only the vertices between the two
    /// endpoints.
    VertexSlice(d::VertexCount, d::VertexCount),
    /// The `IndexSlice` buffer contains a list of indices into the `Mesh` data, so every vertex
    /// attribute does not need to be duplicated, only its position in the `Mesh`.  For example,
    /// when drawing a square, two triangles are needed.  Using only `VertexSlice`, one would need
    /// 6 separate vertices, 3 for each triangle. However, two of the vertices will be identical,
    /// wasting space for the duplicated attributes.  Instead, the `Mesh` can store 4 vertices and
    /// an `IndexSlice` can be used instead.
    IndexSlice(Buffer, d::IndexType, d::IndexCount, d::IndexCount),
}

/// A slice of a mesh, with a given material.
#[deriving(Clone, Show)]
pub struct SubMesh {
    /// `Mesh` this `SubMesh` was created from.
    pub mesh: Mesh,
    /// Slice of the mesh to use.
    pub slice: Slice,
}
