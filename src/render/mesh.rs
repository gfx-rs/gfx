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

/// Vertex attribute descriptor, goes into the vertex shader input
#[deriving(Clone, PartialEq, Show)]
pub struct Attribute {
    pub buffer: super::BufferHandle, // vertex buffer to contain the data
    pub elem_count: a::Count,   // number of elements per vertex
    pub elem_type: a::Type,     // type of a single element
    pub offset: a::Offset,      // offset in bytes to the first vertex
    pub stride: a::Stride,      // stride in bytes between consecutive vertices
    pub name: String,           // a name to match the shader input
}

/// A trait implemented automatically for user vertex structure by
/// `#[vertex_format] attribute
pub trait VertexFormat {
    fn generate(Option<Self>, super::BufferHandle) -> Vec<Attribute>;
}


#[deriving(Clone, Show)]
pub enum PolygonType {
    Point,
    Line,
    LineStrip,
    TriangleList,
    TriangleStrip,
    //Quad,
}

/// Mesh descriptor, as a collection of attributes
#[deriving(Clone, Show)]
pub struct Mesh {
    pub poly_type: PolygonType,
    pub num_vertices: VertexCount,
    pub attributes: Vec<Attribute>,
}

impl Mesh {
    pub fn new(nv: VertexCount) -> Mesh {
        Mesh {
            poly_type: TriangleList,
            num_vertices: nv,
            attributes: Vec::new(),
        }
    }

    pub fn from<V: VertexFormat>(buf: super::BufferHandle, nv: VertexCount) -> Mesh {
        Mesh {
            poly_type: TriangleList,
            num_vertices: nv,
            attributes: VertexFormat::generate(None::<V>, buf),
        }
    }
}

#[deriving(Clone, Show)]
pub enum Slice  {
    VertexSlice(VertexCount, VertexCount),
    IndexSlice(dev::Buffer, ElementCount, ElementCount),
}

/// Slice descriptor with an assigned material
#[deriving(Clone, Show)]
pub struct SubMesh {
    pub mesh: Mesh,
    pub material: MaterialHandle,
    pub slice: Slice,
}
