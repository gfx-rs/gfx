// Copyright 2013 The Gfx-rs Developers.
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

//! Structural types for math library interop

pub type Points = ~[u32];
pub type Lines = ~[[u32,..2]];
pub type Triangles = ~[[u32,..3]];

pub type Vertex2<T> = [T,..2];
pub type Vertex3<T> = [T,..3];
pub type Vertex4<T> = [T,..4];
pub type Matrix2x2<T> = [[T,..2],..2];
pub type Matrix2x3<T> = [[T,..3],..2];
pub type Matrix2x4<T> = [[T,..4],..2];
pub type Matrix3x2<T> = [[T,..2],..3];
pub type Matrix3x3<T> = [[T,..3],..3];
pub type Matrix3x4<T> = [[T,..4],..3];
pub type Matrix4x2<T> = [[T,..2],..4];
pub type Matrix4x3<T> = [[T,..3],..4];
pub type Matrix4x4<T> = [[T,..4],..4];
