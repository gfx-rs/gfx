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

//! High-level, platform independent, bindless rendering API.

#![crate_name = "render"]
#![comment = "A platform independent renderer for gfx-rs."]
#![license = "ASL2"]
#![crate_type = "lib"]
#![deny(missing_doc)]
#![feature(macro_rules, phase)]

#[phase(plugin, link)] extern crate log;
extern crate device;

/// Frontend
pub mod front;
/// Meshes
pub mod mesh;
/// Resources (deprecated)
mod resource {
    mod handle;
}
/// Shaders
pub mod shade;
/// Draw state
pub mod state;
/// Render targets
pub mod target;
