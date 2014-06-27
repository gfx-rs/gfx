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


// Describing shader parameters

pub type Dimension = u8;
pub type IsArray = bool;
pub type IsShadow = bool;
pub type IsMultiSample = bool;
pub type IsRect = bool;

pub enum MatrixFormat {
    ColumnMajor,
    RowMajor,
}

pub enum SamplerType {
    SamplerBuffer,
    Sampler1D(IsArray, IsShadow),
    Sampler2D(IsArray, IsShadow, IsMultiSample, IsRect),
    Sampler3D,
    SamplerCube(IsShadow),
}

pub enum BaseType {
    BaseFloat,
    BaseInt,
    BaseUnsigned,
    BaseBool,
}

pub enum VarType {
    Vector(BaseType, Dimension),
    Matrix(MatrixFormat, Dimension, Dimension),
}


// Describing object data

pub enum Stage {
    Vertex,
    Geometry,
    Fragment,
}


// Describing program data

pub type Location = uint;

pub enum UniformValue {
    ValueInt(i32),
    ValueFloat(f32),
    ValueIntVec([i32, ..4]),
    ValueFloatVec([f32, ..4]),
    ValueMatrix([[f32, ..4], ..4]),
}

pub struct Attribute {
    pub name: String,
    pub location: uint, // Vertex attribute binding
    pub count: uint,
    pub var_type: VarType,
}

pub struct UniformVar {
    pub name: String,
    pub location: Location,
    pub count: uint,
    pub var_type: VarType,
    pub active_value: Cell<UniformValue>,
}

pub struct BlockVar {
    pub name: String,
    pub size: uint,
    pub usage: u8, // Bit flags for each shader stage
    pub active_slot: Cell<u8>, // Active uniform block binding
}

pub struct SamplerVar {
    pub name: String,
    pub value_type: BaseType,
    pub sampler_type: SamplerType,
    pub active_slot: Cell<u8>, // Active texture binding
}

pub struct ProgramMeta {
    pub name: super::dev::Program,
    pub attributes: Vec<Attribute>,
    pub uniforms: Vec<UniformVar>,
    pub blocks: Vec<BlockVar>,
    pub textures: Vec<SamplerVar>,
}
