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
//!
//! Nothing interesting here for users.

#![allow(missing_doc)]

pub type Count = u8;    // only value 1 to 4 are supported
pub type Offset = u32;  // can point in the middle of the buffer
pub type Stride = u8;   // I don't believe HW supports more

#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum SignFlag {
    Signed,
    Unsigned,
}

#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum IntSubType {
    IntRaw,         // un-processed integer
    IntNormalized,  // normalized either to [0,1] or [-1,1] depending on the sign flag
    IntAsFloat,     // converted to float on the fly by the hardware
}

#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum IntSize {
    U8,
    U16,
    U32,
}

#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum FloatSubType {
    FloatDefault,    // 32-bit
    FloatPrecision,  // 64-bit
}

#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
#[repr(u8)]
pub enum FloatSize {
    F16,
    F32,
    F64,
}

#[deriving(Eq, Ord, PartialEq, PartialOrd, Hash, Clone, Show)]
pub enum Type {
    Int(IntSubType, IntSize, SignFlag),
    Float(FloatSubType, FloatSize),
    Special,
}

impl Type {
    pub fn is_compatible(&self, bt: super::shade::BaseType) -> Result<(), ()> {
        use s = super::shade;
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
