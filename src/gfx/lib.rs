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

#![crate_id = "github.com/bjz/gfx-rs#gfx:0.1"]
#![comment = "A lightweight graphics device manager for Rust"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(macro_rules, phase)]
#[phase(plugin, link)] extern crate log;
extern crate libc;
extern crate device;

// public re-exports
pub use render::{BufferHandle, MeshHandle, SurfaceHandle, TextureHandle, SamplerHandle, ProgramHandle, EnvirHandle};
pub use render::Renderer;
pub use MeshSlice = render::mesh::Slice;
pub use render::mesh::{VertexCount, ElementCount, VertexSlice, IndexSlice};
pub use Environment = render::envir::Storage;
pub use render::envir::{BlockVar, UniformVar, TextureVar};
pub use render::target::Frame;
pub use Device = device::Server;
pub use device::target::{Color, ClearData, Plane, TextureLayer, TextureLevel};
pub use device::{GraphicsContext, InitError};
pub use device::shade::{UniformValue, ValueI32, ValueF32, ValueI32Vec, ValueF32Vec, ValueF32Matrix};

mod render;
pub mod platform;

#[allow(visible_private_types)]
pub fn start<Api, P: GraphicsContext<Api>>(graphics_context: P, options: device::Options)
        -> Result<(Renderer, Device<P, device::Device>), InitError> {
    device::init(graphics_context, options).map(|(client, server)| {
        (Renderer::new(client), server)
    })
}
