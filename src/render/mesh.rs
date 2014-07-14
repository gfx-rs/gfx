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
#[deriving(Clone, Show)]
pub struct Attribute {
    pub buffer: super::BufferHandle, // vertex buffer to contain the data
    pub elem_count: a::Count,   // number of elements per vertex
    pub elem_type: a::Type,     // type of a single element
    pub offset: a::Offset,      // offset in bytes to the first vertex
    pub stride: a::Stride,      // stride in bytes between consecutive vertices
    pub name: String,           // a name to match the shader input
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
}

pub enum ComponentType {
    U8,
    U8n,
    U8f,
    I8,
    I8n,
    I8f,
    U16,
    U16n,
    U16f,
    I16,
    I16n,
    I16f,
    U32,
    U32n,
    U32f,
    I32,
    I32n,
    I32f,
    F16,
    F32,
    F64,
    F64d,
}

impl ComponentType {
    pub fn decode(&self) -> (u8, a::Type) {
        match *self {
            U8     => (1, a::Int(a::IntRaw,        a::U8,  a::Unsigned)),
            U8n    => (1, a::Int(a::IntNormalized, a::U8,  a::Unsigned)),
            U8f    => (1, a::Int(a::IntAsFloat,    a::U8,  a::Unsigned)),
            I8     => (1, a::Int(a::IntRaw,        a::U8,  a::Signed)),
            I8n    => (1, a::Int(a::IntNormalized, a::U8,  a::Signed)),
            I8f    => (1, a::Int(a::IntAsFloat,    a::U8,  a::Signed)),
            U16    => (2, a::Int(a::IntRaw,        a::U16, a::Unsigned)),
            U16n   => (2, a::Int(a::IntNormalized, a::U16, a::Unsigned)),
            U16f   => (2, a::Int(a::IntAsFloat,    a::U16, a::Unsigned)),
            I16    => (2, a::Int(a::IntRaw,        a::U16, a::Signed)),
            I16n   => (2, a::Int(a::IntNormalized, a::U16, a::Signed)),
            I16f   => (2, a::Int(a::IntAsFloat,    a::U16, a::Signed)),
            U32    => (4, a::Int(a::IntRaw,        a::U32, a::Unsigned)),
            U32n   => (4, a::Int(a::IntNormalized, a::U32, a::Unsigned)),
            U32f   => (4, a::Int(a::IntAsFloat,    a::U32, a::Unsigned)),
            I32    => (4, a::Int(a::IntRaw,        a::U32, a::Signed)),
            I32n   => (4, a::Int(a::IntNormalized, a::U32, a::Signed)),
            I32f   => (4, a::Int(a::IntAsFloat,    a::U32, a::Signed)),
            F16    => (2, a::Float(a::FloatDefault,   a::F16)),
            F32    => (4, a::Float(a::FloatDefault,   a::F32)),
            F64    => (8, a::Float(a::FloatDefault,   a::F64)),
            F64d   => (8, a::Float(a::FloatPrecision, a::F64)),
        }
    }
}

/// A helper class to populate Mesh attributes
pub struct Constructor {
    buffer: super::BufferHandle,
    offset: a::Offset,
    attributes: Vec<Attribute>,
}

impl Constructor {
    pub fn new(handle: super::BufferHandle) -> Constructor {
        Constructor {
            buffer: handle,
            offset: 0,
            attributes: Vec::new(),
        }
    }

    pub fn add(mut self, name: &str, count: a::Count, format: ComponentType) -> Constructor {
        let (size, e_type) = format.decode();
        self.attributes.push(Attribute {
            buffer: self.buffer,
            elem_count: count,
            elem_type: e_type,
            offset: self.offset,
            stride: 0,
            name: name.to_string(),
        });
        self.offset += (count as a::Offset) * (size as a::Offset);
        self
    }

    fn finalize(&mut self) {
        for at in self.attributes.mut_iter() {
            at.stride = self.offset as a::Stride;
        }
    }

    pub fn embed_to(mut self, mesh: &mut Mesh) {
        self.finalize();
        mesh.attributes.push_all(self.attributes.as_slice());
    }

    pub fn complete(mut self, nv: VertexCount) -> Mesh {
        self.finalize();
        Mesh {
            poly_type: TriangleList,
            num_vertices: nv,
            attributes: self.attributes,
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
