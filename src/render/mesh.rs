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

    pub fn embed(&mut self, mut builder: Builder) {
        builder.finalize();
        self.attributes.push_all(builder.attributes.as_slice());
    }
}


/// The numeric type of a vertex's components.
pub enum ComponentType {
    /// An 8-bit unsigned integer component.
    U8,
    /// An 8-bit unsigned integer component normalized to `[0, 1]`.
    U8N,
    /// An 8-bit unsigned integer component that is converted to a 32-bit float
    /// at runtime by the hardware.
    U8F,
    /// An 8-bit signed integer component.
    I8,
    /// An 8-bit signed integer component normalized to `[-1, 1]`.
    I8N,
    /// An 8-bit signed integer component that is converted to a 32-bit float at
    /// runtime by the hardware.
    I8F,
    /// A 16-bit unsigned integer component.
    U16,
    /// A 16-bit unsigned integer component normalized to `[-1, 1]`.
    U16N,
    /// A 16-bit unsigned integer component that is converted to a 32-bit float
    /// at runtime by the hardware.
    U16F,
    /// A 16-bit signed integer component.
    I16,
    /// A 16-bit signed integer component normalized to `[-1, 1]`.
    I16N,
    /// A 16-bit signed integer component that is converted to a 32-bit float at
    /// runtime by the hardware.
    I16F,
    /// A 32-bit unsigned integer component.
    U32,
    /// A 32-bit unsigned integer component normalized to `[-1, 1]`.
    U32N,
    /// A 32-bit unsigned integer component that is converted to a 32-bit float
    /// at runtime by the hardware.
    U32F,
    /// A 32-bit signed integer component.
    I32,
    /// A 32-bit signed integer component normalized to `[-1, 1]`.
    I32N,
    /// A 32-bit signed integer component that is converted to a 32-bit float at
    /// runtime by the hardware.
    I32F,
    /// A 16-bit (half precision) floating point component that is converted to
    /// a 32-bit float at runtime by the hardware.
    F16,
    /// A 32-bit (single precision) floating point component.
    F32,
    /// A 64-bit (double precision) floating point componentthat is converted to
    /// a 32-bit float at runtime by the hardware.
    F64,
    /// A 64-bit (double precision) floating point component.
    F64P,
}

/// The number of bytes in a vertex component.
pub type ByteSize = u8;

impl ComponentType {
    pub fn decode(&self) -> (ByteSize, a::Type) {
        match *self {
            U8     => (1, a::Int(a::IntRaw,        a::U8,  a::Unsigned)),
            U8N    => (1, a::Int(a::IntNormalized, a::U8,  a::Unsigned)),
            U8F    => (1, a::Int(a::IntAsFloat,    a::U8,  a::Unsigned)),
            I8     => (1, a::Int(a::IntRaw,        a::U8,  a::Signed)),
            I8N    => (1, a::Int(a::IntNormalized, a::U8,  a::Signed)),
            I8F    => (1, a::Int(a::IntAsFloat,    a::U8,  a::Signed)),
            U16    => (2, a::Int(a::IntRaw,        a::U16, a::Unsigned)),
            U16N   => (2, a::Int(a::IntNormalized, a::U16, a::Unsigned)),
            U16F   => (2, a::Int(a::IntAsFloat,    a::U16, a::Unsigned)),
            I16    => (2, a::Int(a::IntRaw,        a::U16, a::Signed)),
            I16N   => (2, a::Int(a::IntNormalized, a::U16, a::Signed)),
            I16F   => (2, a::Int(a::IntAsFloat,    a::U16, a::Signed)),
            U32    => (4, a::Int(a::IntRaw,        a::U32, a::Unsigned)),
            U32N   => (4, a::Int(a::IntNormalized, a::U32, a::Unsigned)),
            U32F   => (4, a::Int(a::IntAsFloat,    a::U32, a::Unsigned)),
            I32    => (4, a::Int(a::IntRaw,        a::U32, a::Signed)),
            I32N   => (4, a::Int(a::IntNormalized, a::U32, a::Signed)),
            I32F   => (4, a::Int(a::IntAsFloat,    a::U32, a::Signed)),
            F16    => (2, a::Float(a::FloatDefault,   a::F16)),
            F32    => (4, a::Float(a::FloatDefault,   a::F32)),
            F64    => (8, a::Float(a::FloatDefault,   a::F64)),
            F64P   => (8, a::Float(a::FloatPrecision, a::F64)),
        }
    }
}

/// A helper class to populate Mesh attributes
pub struct Builder {
    buffer: super::BufferHandle,
    offset: a::Offset,
    attributes: Vec<Attribute>,
}

impl Builder {
    pub fn new(handle: super::BufferHandle) -> Builder {
        Builder {
            buffer: handle,
            offset: 0,
            attributes: Vec::new(),
        }
    }

    pub fn add(mut self, name: &str, count: a::Count, format: ComponentType) -> Builder {
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
