// Copyright 2017 The Gfx-rs Developers.
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

//! Input Assembler(IA) stage description.

use {format, state as s};
use {Backend, Primitive};

/// Shader binding location.
pub type Location = u32;
/// Index of a vertex buffer.
pub type BufferIndex = u32;
/// Offset of an attribute from the start of the buffer, in bytes
pub type ElemOffset = u32;
/// Offset between attribute values, in bytes
pub type ElemStride = u32;
/// The number of instances between each subsequent attribute value
pub type InstanceRate = u8;
/// An offset inside a vertex buffer, in bytes.
pub type BufferOffset = usize;

/// A struct element descriptor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Element<F> {
    /// Element format
    pub format: F,
    /// Offset from the beginning of the container, in bytes
    pub offset: ElemOffset,
}

/// Vertex buffer descriptor
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct VertexBufferDesc {
    /// Total container size, in bytes.
    /// Specifies the byte distance between two consecutive elements.
    pub stride: ElemStride,
    /// Rate of the input for the given buffer
    pub rate: InstanceRate,
}

/// PSO vertex attribute descriptor
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct AttributeDesc {
    /// Attribute binding location in the shader.
    location: Location,
    /// Index of the associated vertex buffer descriptor.
    binding: BufferIndex,
    /// Attribute element description.
    element: Element<format::Format>,
}

/// A complete set of vertex buffers to be used for vertex import in PSO.
#[derive(Clone, Debug)]
pub struct VertexBufferSet<'a, B: Backend>(
    /// Array of buffer handles with offsets in them
    pub Vec<(&'a B::Buffer, BufferOffset)>,
);

impl<'a, B: Backend> VertexBufferSet<'a, B> {
    /// Create an empty set
    pub fn new() -> VertexBufferSet<'a, B> {
        VertexBufferSet(Vec::new())
    }
}
