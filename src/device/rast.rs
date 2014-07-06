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
pub enum DrawCwFlag {
    DrawCw,
    CullCw,
}

#[deriving(Clone, PartialEq, Show)]
pub enum DrawCcwFlag {
    DrawCcw,
    CullCcw,
}

pub type LineWidth = f32;

#[deriving(Clone, PartialEq, Show)]
pub enum Primitive {
    Point,
    Line(LineWidth),
    Fill(DrawCwFlag, DrawCcwFlag),
}


#[deriving(Clone, PartialEq, Show)]
enum LessFlag {
    Less,
    NoLess,
}

#[deriving(Clone, PartialEq, Show)]
enum EqualFlag {
    Equal,
    NoEqual,
}

#[deriving(Clone, PartialEq, Show)]
enum GreaterFlag {
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
    fun: Comparison,
}

//TODO
#[deriving(Clone, PartialEq, Show)]
pub struct Blend;

pub type StencilMask = super::target::Stencil;

#[deriving(Clone, PartialEq, Show)]
pub struct Mask {
    stencil_cw: StencilMask,
    stencil_ccw: StencilMask,
    depth: bool,
}

#[deriving(Clone, PartialEq, Show)]
pub struct DrawState {
    primitive: Primitive,
    stencil: Option<Stencil>,
    depth: Option<Depth>,
    blend: Option<Blend>,
    mask: Mask,
}
