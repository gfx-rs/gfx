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

#![allow(missing_doc)]

/// Number of elements per attribute, only 1 to 4 are supported
pub type Count = u8;
/// Offset of an attribute from the start of the buffer, in bytes
pub type Offset = u32;
/// Offset between attribute values, in bytes
pub type Stride = u8;
/// The number of instances between each subsequent attribute value
pub type InstanceRate = u8;

/// The signedness of an attribute.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum SignFlag {
    Signed,
    Unsigned,
}

/// Describes how an integer value is interpreted by the shader.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum IntSubType {
    IntRaw,         // un-processed integer
    IntNormalized,  // normalized either to [0,1] or [-1,1] depending on the sign flag
    IntAsFloat,     // converted to float on the fly by the hardware
}

/// The size of an integer attribute, in bits.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum IntSize {
    U8,
    U16,
    U32,
}

/// Type of a floating point attribute on the shader side.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum FloatSubType {
    FloatDefault,    // 32-bit
    FloatPrecision,  // 64-bit
}

/// The size of a floating point attribute, in bits.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum FloatSize {
    F16,
    F32,
    F64,
}

/// The type of an attribute.
#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub enum Type {
    Int(IntSubType, IntSize, SignFlag),
    Float(FloatSubType, FloatSize),
    Special,
}

impl Type {
    /// Check if the attribute is compatible with a particular shader type.
    pub fn is_compatible(&self, bt: super::shade::BaseType) -> Result<(), ()> {
        use super::shade as s;
        match (*self, bt) {
            (Int(IntRaw, _, _), s::BaseI32) => Ok(()),
            (Int(IntRaw, _, Unsigned), s::BaseU32) => Ok(()),
            (Int(IntRaw, _, _), _) => Err(()),
            (Int(_, _, _), s::BaseF32) => Ok(()),
            (Int(_, _, _), _) => Err(()),
            (Float(_, _), s::BaseF32) => Ok(()),
            (Float(FloatPrecision, F64), s::BaseF64) => Ok(()),
            (Float(_, _), _) => Err(()),
            (_, s::BaseF64) => Err(()),
            (_, s::BaseBool) => Err(()),
            _ => Err(()),
        }
    }
}
