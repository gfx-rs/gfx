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


#[deriving(Clone, PartialEq, Show)]
pub enum FrontType {
    Cw,
    Ccw,
}

pub type LineWidth = f32;
pub type OffsetFactor = f32;
pub type OffsetUnits = u32;

#[deriving(Clone, PartialEq, Show)]
pub enum OffsetType {
    NoOffset,
    Offset(OffsetFactor, OffsetUnits),
}

#[deriving(Clone, PartialEq, Show)]
pub enum FrontFlag {
    DrawFront,
    CullFront,
}

#[deriving(Clone, PartialEq, Show)]
pub enum BackFlag {
    DrawBack,
    CullBack,
}

#[deriving(Clone, PartialEq, Show)]
pub enum RasterMethod {
    Point,
    Line(LineWidth),
    Fill(FrontFlag, BackFlag),
}

/// Primitive rasterization state. Note that GL allows different raster
/// method to be used for front and back, while this abstraction does not.
#[deriving(Clone, PartialEq, Show)]
pub struct Primitive {
    pub front_face: FrontType,
    pub method: RasterMethod,
    pub offset: OffsetType,
}


#[deriving(Clone, PartialEq, Show)]
pub enum LessFlag {
    Less,
    NoLess,
}

#[deriving(Clone, PartialEq, Show)]
pub enum EqualFlag {
    Equal,
    NoEqual,
}

#[deriving(Clone, PartialEq, Show)]
pub enum GreaterFlag {
    Greater,
    NoGreater,
}

#[deriving(Clone, PartialEq, Show)]
pub struct Comparison(pub LessFlag, pub EqualFlag, pub GreaterFlag);

//TODO
#[deriving(Clone, PartialEq, Show)]
pub struct Stencil;

#[deriving(Clone, PartialEq, Show)]
pub struct Depth {
    pub fun: Comparison,
    pub write: bool,
}

#[deriving(Clone, PartialEq, Show)]
pub enum Equation {
    FuncAdd,
    FuncSub,
    FuncRevSub,
    FuncMin,
    FuncMax,
}

#[deriving(Clone, PartialEq, Show)]
pub enum InverseFlag {
    Normal,
    Inverse,
}

#[deriving(Clone, PartialEq, Show)]
pub enum BlendValue {
    Zero,
    SourceColor,
    SourceAlpha,
    SourceAlphaSaturated,
    DestColor,
    DestAlpha,
    ConstColor,
    ConstAlpha,
}

#[deriving(Clone, PartialEq, Show)]
pub struct Factor(pub InverseFlag, pub BlendValue);

#[deriving(Clone, PartialEq, Show)]
pub struct BlendChannel {
    pub equation: Equation,
    pub source: Factor,
    pub destination: Factor,
}

#[deriving(Clone, PartialEq, Show)]
pub struct Blend {
    pub color: BlendChannel,
    pub alpha: BlendChannel,
    pub value: super::target::Color,
}
