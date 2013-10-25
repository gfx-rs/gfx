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

extern mod gl;

/// Index Buffer Object
pub struct Ibo;

/// Vertex Buffer Object
pub struct Vbo;

/// Uniform Buffer Object
pub struct Ubo;

/// Shader Program
pub struct Program;

/// A graphics device manager
pub struct Gfx {
    ibos: ~[Ibo],
    vbos: ~[Vbo],
    ubos: ~[Ubo],
    programs: ~[Program],
}

impl Gfx {
    /// Initialise a new graphics device manager
    pub fn new() -> Gfx {
        Gfx {
            ibos: ~[],
            vbos: ~[],
            ubos: ~[],
            programs: ~[],
        }
    }
}

impl Drop for Gfx {
    fn drop(&mut self) {
        // Clean up all the things
    }
}
