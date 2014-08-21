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

#![crate_name = "tests"]
#![feature(phase)]

#[phase(plugin)]
extern crate gfx_macros;

mod secret_lib {
    extern crate gfx_ = "gfx";
    extern crate device_ = "device";
    pub use self::gfx_ as gfx;
    pub use self::device_ as device;
}

pub mod shaders_macro;
pub mod shader_param;
pub mod vertex_format;
