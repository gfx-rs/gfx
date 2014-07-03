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

use std::cell::Cell;
use std::fmt;

// Describing shader parameters

pub type Dimension = u8;

#[deriving(Show)]
pub enum IsArray { Array, NoArray }

#[deriving(Show)]
pub enum IsShadow { Shadow, NoShadow }

#[deriving(Show)]
pub enum IsMultiSample { MultiSample, NoMultiSample }

#[deriving(Show)]
pub enum IsRect { Rect, NoRect }

#[deriving(Show)]
pub enum MatrixFormat { ColumnMajor, RowMajor }

#[deriving(Show)]
pub enum SamplerType {
    SamplerBuffer,
    Sampler1D(IsArray, IsShadow),
    Sampler2D(IsArray, IsShadow, IsMultiSample, IsRect),
    Sampler3D,
    SamplerCube(IsShadow),
}

#[deriving(Show)]
pub enum BaseType {
    BaseF32,
    BaseF64,
    BaseI32,
    BaseU32,
    BaseBool,
}

#[deriving(Show)]
pub enum ContainerType {
    Single,
    Vector(Dimension),
    Matrix(MatrixFormat, Dimension, Dimension),
}

// Describing object data

#[deriving(Show)]
pub enum Stage {
    Vertex,
    Geometry,
    Fragment,
}

// Describing program data

pub type Location = uint;

// unable to derive anything for fixed arrays
pub enum UniformValue {
    ValueUninitialized,
    ValueI32(i32),
    ValueF32(f32),
    ValueI32Vec([i32, ..4]),
    ValueF32Vec([f32, ..4]),
    ValueF32Matrix([[f32, ..4], ..4]),
}

impl UniformValue {
    pub fn is_valid(&self) -> bool {
        match *self {
            ValueUninitialized => false,
            _ => true,
        }
    }

    pub fn is_same_type(&self, other: &UniformValue) -> bool {
        match (*self, *other) {
            (ValueI32(_), ValueI32(_)) => true,
            (ValueF32(_), ValueF32(_)) => true,
            (ValueI32Vec(_), ValueI32Vec(_)) => true,
            (ValueF32Vec(_), ValueF32Vec(_)) => true,
            (ValueF32Matrix(_), ValueF32Matrix(_)) => true,
            _ => false,
        }
    }
}

/*  // the type has Copy implemented implicitly, until we introduce boxed fields
impl Clone for UniformValue {
    fn clone(&self) -> UniformValue {
        match *self {
            ValueUninitialized  => ValueUninitialized,
            ValueI32(val)       => ValueI32(val),
            ValueF32(val)       => ValueF32(val),
            ValueI32Vec(v)      => ValueI32Vec([v[0], v[1], v[2], v[3]]),
            ValueF32Vec(v)      => ValueF32Vec([v[0], v[1], v[2], v[3]]),
            ValueF32Matrix(v)   => ValueF32Matrix(box [
                [v[0][0], v[0][1], v[0][2], v[0][3]],
                [v[1][0], v[1][1], v[1][2], v[1][3]],
                [v[2][0], v[2][1], v[2][2], v[2][3]],
                [v[3][0], v[3][1], v[3][2], v[3][3]]
                ])),
        }
    }
}*/

impl fmt::Show for UniformValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ValueUninitialized    => write!(f, "ValueUninitialized"),
            ValueI32(x)           => write!(f, "ValueI32({})", x),
            ValueF32(x)           => write!(f, "ValueF32({})", x),
            ValueI32Vec(ref v)    => write!(f, "ValueI32Vec({})", v.as_slice()),
            ValueF32Vec(ref v)    => write!(f, "ValueF32Vec({})", v.as_slice()),
            ValueF32Matrix(ref m) => {
                try!(write!(f, "ValueF32Matrix("));
                for v in m.iter() {
                    try!(write!(f, "{}", v.as_slice()));
                }
                write!(f, ")")
            },
        }
    }
}

#[deriving(Show)]
pub struct Attribute {
    pub name: String,
    pub location: uint, // Vertex attribute binding
    pub count: uint,
    pub base_type: BaseType,
    pub container: ContainerType,
}

#[deriving(Show)]
pub struct UniformVar {
    pub name: String,
    pub location: Location,
    pub count: uint,
    pub base_type: BaseType,
    pub container: ContainerType,
    pub active_value: Cell<UniformValue>,
}

#[deriving(Show)]
pub struct BlockVar {
    pub name: String,
    pub size: uint,
    pub usage: u8, // Bit flags for each shader stage
    pub active_slot: Cell<u8>, // Active uniform block binding
}

#[deriving(Show)]
pub struct SamplerVar {
    pub name: String,
    pub location: Location,
    pub base_type: BaseType,
    pub sampler_type: SamplerType,
    pub active_slot: Cell<u8>, // Active texture binding
}

#[deriving(Show)]
pub struct ProgramMeta {
    pub name: super::dev::Program,
    pub attributes: Vec<Attribute>,
    pub uniforms: Vec<UniformVar>,
    pub blocks: Vec<BlockVar>,
    pub textures: Vec<SamplerVar>,
}

#[deriving(Show)]
pub enum CompatibilityError {
    ErrorArraySize,
    ErrorBaseType,
    ErrorContainer,
}

impl UniformVar {
    pub fn is_compatible(&self, value: &UniformValue) -> Result<(), CompatibilityError> {
        if self.count != 1 {
            return Err(ErrorArraySize)
        }
        match (self.base_type, self.container, *value) {
            (BaseI32, Single, ValueI32(_)) => Ok(()),
            (BaseF32, Single, ValueF32(_)) => Ok(()),
            (BaseF32, Vector(4), ValueF32Vec(_)) => Ok(()),
            (BaseF32, Vector(_), ValueF32Vec(_)) => Err(ErrorContainer),
            (BaseI32, Vector(4), ValueI32Vec(_)) => Ok(()),
            (BaseI32, Vector(_), ValueI32Vec(_)) => Err(ErrorContainer),
            (BaseF32, Matrix(_, 4,4), ValueF32Matrix(_)) => Ok(()),
            (BaseF32, Matrix(_, _,_), ValueF32Matrix(_)) => Err(ErrorContainer),
            _ => Err(ErrorBaseType)
        }
    }
}
