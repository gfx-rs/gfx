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

use device::dev;
use a = device::attrib;

pub type MaterialHandle = int;  //placeholder
pub type VertexCount = u16;
pub type ElementCount = u16;

/// Describes a single attribute of a vertex buffer, including its type, name, etc.
#[deriving(Clone, PartialEq, Show)]
pub struct Attribute {
    /// Vertex buffer to contain the data
    pub buffer: super::BufferHandle,
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
    fn generate(Option<Self>, super::BufferHandle) -> Vec<Attribute>;
}


/// Describes what geometric primitives are created from vertex data.
#[deriving(Clone, Show)]
pub enum PrimitiveType {
    /// Each vertex represents a single point.
    Point,
    /// Each pair of vertices represent a single line segment. For example, with `[a, b, c, d,
    /// e]`, `a` and `b` form a line, `c` and `d` form a line, and `e` is discarded.
    Line,
    /// Every two consecutive vertices represent a single line segment. Visually forms a "path" of
    /// lines, as they are all connected. For example, with `[a, b, c]`, `a` and `b` form a line
    /// line, and `b` and `c` form a line.
    LineStrip,
    /// Each triplet of vertices represent a single triangle. For example, with `[a, b, c, d, e]`,
    /// `a`, `b`, and `c` form a triangle, `d` and `e` are discarded.
    TriangleList,
    /// Every three consecutive vertices represent a single triangle. For example, with `[a, b, c,
    /// d]`, `a`, `b`, and `c` form a triangle, and `b`, `c`, and `d` form a triangle.
    TriangleStrip,
    //Quad,
}

/// Describes geometry to render.
///
/// The best way to create a `Mesh` is to use the `Builder` in this module.
#[deriving(Clone, Show)]
pub struct Mesh {
    /// What primitives to form out of the vertex data.
    pub prim_type: PrimitiveType,
    /// Number of vertices in the mesh.
    pub num_vertices: VertexCount,
    /// Vertex attributes to use.
    pub attributes: Vec<Attribute>,
}

impl Mesh {
    /// Create a new mesh, which is a `TriangleList` with no attributes and `nv` vertices.
    pub fn new(nv: VertexCount) -> Mesh {
        Mesh {
            prim_type: TriangleList,
            num_vertices: nv,
            attributes: Vec::new(),
        }
    }

    /// Create a new `Mesh` from a struct that implements `VertexFormat` and a buffer.
    pub fn from<V: VertexFormat>(buf: super::BufferHandle, nv: VertexCount) -> Mesh {
        Mesh {
            prim_type: TriangleList,
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
    VertexSlice(VertexCount, VertexCount),
    /// The `IndexSlice` buffer contains a list of indices into the `Mesh` data, so every vertex
    /// attribute does not need to be duplicated, only its position in the `Mesh`.  For example,
    /// when drawing a square, two triangles are needed.  Using only `VertexSlice`, one would need
    /// 6 separate vertices, 3 for each triangle. However, two of the vertices will be identical,
    /// wasting space for the duplicated attributes.  Instead, the `Mesh` can store 4 vertices and
    /// an `IndexSlice` can be used instead.
    IndexSlice(super::BufferHandle, ElementCount, ElementCount),
}

/// A slice of a mesh, with a given material.
#[deriving(Clone, Show)]
pub struct SubMesh {
    pub mesh: Mesh,
    pub material: MaterialHandle,
    pub slice: Slice,
}
