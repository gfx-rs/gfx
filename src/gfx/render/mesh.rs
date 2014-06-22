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

pub type MaterialHandle = int;  //placeholder
pub type VertexCount = u16;
pub type ElementCount = u16;

pub static MAX_ATTRIBUTES: uint = 8;


/// Vertex attribute descriptor, goes into the vertex shader input
pub struct Attribute {
    pub buffer: dev::Buffer,
    pub offset: u32, // can be the middle of the buffer
    pub stride: u8, // should be enough
    pub is_normalized: bool, // treat unsigned as fixed-point
    pub is_interpolated: bool, // allow shader interpolation
    pub name: (), // a real name, should be a String
}

pub static ATTRIB_EMPTY: Attribute = Attribute {
    buffer: 0,
    offset: 0,
    stride: 0,
    is_normalized: false,
    is_interpolated: false,
    name: (),
};

pub enum PolygonType {
    Point,
    Line,
    LineStrip,
    TriangleList,
    TriangleStrip,
    //Quad,
}

/// Mesh descriptor, as a collection of attributes
pub struct Mesh {
    pub poly_type       : PolygonType,
    pub num_vertices    : VertexCount,
    pub attributes      : [Attribute, ..MAX_ATTRIBUTES],
}

impl Mesh {
    pub fn new(nv: VertexCount) -> Mesh {
        Mesh {
            poly_type: TriangleList,
            num_vertices: nv,
            attributes: [ATTRIB_EMPTY, ..MAX_ATTRIBUTES],
        }
    }
}


pub enum Slice  {
    VertexSlice(VertexCount, VertexCount),
    IndexSlice(dev::Buffer, ElementCount, ElementCount),
}

/// Slice descriptor with an assigned material
pub struct SubMesh {
    pub mesh: Mesh,
    pub material: MaterialHandle,
    pub slice: Slice,
}
