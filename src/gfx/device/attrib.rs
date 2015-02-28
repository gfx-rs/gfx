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

//! Vertex attribute types.

#![allow(missing_docs)]

use device::shade::BaseType;

/// Number of elements per attribute, only 1 to 4 are supported
pub type Count = u8;
/// Offset of an attribute from the start of the buffer, in bytes
pub type Offset = u32;
/// Offset between attribute values, in bytes
pub type Stride = u8;
/// The number of instances between each subsequent attribute value
pub type InstanceRate = u8;

/// The signedness of an attribute.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum SignFlag {
    Signed,
    Unsigned,
}

/// Describes how an integer value is interpreted by the shader.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum IntSubType {
    Raw,         // un-processed integer
    Normalized,  // normalized either to [0,1] or [-1,1] depending on the sign flag
    AsFloat,     // converted to float on the fly by the hardware
}

/// The size of an integer attribute, in bits.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum IntSize {
    U8,
    U16,
    U32,
}

/// Type of a floating point attribute on the shader side.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum FloatSubType {
    Default,    // 32-bit
    Precision,  // 64-bit
}

/// The size of a floating point attribute, in bits.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum FloatSize {
    F16,
    F32,
    F64,
}

/// The type of an attribute.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub enum Type {
    Int(IntSubType, IntSize, SignFlag),
    Float(FloatSubType, FloatSize),
    Special,
}

impl Type {
    /// Check if the attribute is compatible with a particular shader type.
    pub fn is_compatible(&self, bt: BaseType) -> Result<(), ()> {
        match (*self, bt) {
            (Type::Int(IntSubType::Raw, _, _), BaseType::I32) => Ok(()),
            (Type::Int(IntSubType::Raw, _, SignFlag::Unsigned), BaseType::U32) => Ok(()),
            (Type::Int(IntSubType::Raw, _, _), _) => Err(()),
            (Type::Int(_, _, _), BaseType::F32) => Ok(()),
            (Type::Int(_, _, _), _) => Err(()),
            (Type::Float(_, _), BaseType::F32) => Ok(()),
            (Type::Float(FloatSubType::Precision, FloatSize::F64), BaseType::F64) => Ok(()),
            (Type::Float(_, _), _) => Err(()),
            (_, BaseType::F64) => Err(()),
            (_, BaseType::Bool) => Err(()),
            _ => Err(()),
        }
    }

    /// Return the size of the type in bytes.
    pub fn get_size(&self) -> u8 {
        match *self {
            Type::Int(_, IntSize::U8, _) => 1,
            Type::Int(_, IntSize::U16, _) => 2,
            Type::Int(_, IntSize::U32, _) => 4,
            Type::Float(_, FloatSize::F16) => 2,
            Type::Float(_, FloatSize::F32) => 4,
            Type::Float(_, FloatSize::F64) => 8,
            Type::Special => 0,
        }
    }
}

/// Complete format of a vertex attribute.
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
pub struct Format {
    /// Number of elements per vertex
    pub elem_count: Count,
    /// Type of a single element
    pub elem_type: Type,
    /// Offset in bytes to the first vertex
    pub offset: Offset,
    /// Stride in bytes between consecutive vertices
    pub stride: Stride,
    /// Instance rate per vertex
    pub instance_rate: InstanceRate,
}
