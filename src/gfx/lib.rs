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

//! ~~~rust
//! extern crate gfx;
//!
//! #[start]
//! fn start(argc: int, argv: **u8) -> int {
//!     native::start(argc, argv, main)
//! }
//!
//! fn main() {
//!     // spawn render task
//!     let (renderer, mut device) = gfx::start(()).unwrap();
//!
//!     // spawn game task
//!     spawn(proc {
//!         let _ = renderer; // do stuff with renderer
//!         loop {}
//!     })
//!
//!     loop {
//!         device.update(); // update device
//!     }
//! }
//! ~~~
//!
//! ~~~
//!     Render Task        |           Main Platform Thread             |         User Task
//!                        |                                            |
//! +----------------+     |                      +----------------+    |
//! |                |<----- device::Reply -------|                |    |
//! | device::Client |     |                      | device::Server |    |
//! |                |------ device::Request ---->|                |    |
//! +----------------+     |                      +----------------+    |
//!                        |                                            |
//!                        |                                            |     +----------------+
//!                        |<------------- render::Request -------------------|                |
//!                        |                                            |     | render::Client |
//!                        |-------------- render::Reply -------------------->|                |
//!                        |                                            |     +----------------+
//!                        |                                            |
//! ~~~

#![crate_id = "github.com/bjz/gfx-rs#gfx:0.1"]
#![comment = "A lightweight graphics device manager for Rust"]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(macro_rules, phase)]
#[phase(plugin, link)] extern crate log;
extern crate libc;

// public re-exports
pub use render::{BufferHandle, MeshHandle, SurfaceHandle, TextureHandle, SamplerHandle, ProgramHandle, EnvirHandle};
pub use Renderer = render::Client;
pub use MeshSlice = render::mesh::Slice;
pub use render::mesh::{VertexCount, ElementCount, VertexSlice, IndexSlice};
pub use Environment = render::envir::Storage;
pub use render::envir::{BlockVar, UniformVar, TextureVar};
pub use render::target::{ClearData, Plane, Frame, TextureLayer, TextureLevel};
pub use Device = device::Server;
pub use device::{Color, InitError};
pub use device::shade::{UniformValue, ValueI32, ValueF32, ValueI32Vec, ValueF32Vec, ValueF32Matrix};
pub use platform::GraphicsContext;


pub type Options<'a> = &'a platform::GlProvider;

mod device;
mod render;
pub mod platform;

#[allow(visible_private_types)]
pub fn start<Api, P: GraphicsContext<Api>>(graphics_context: P, options: Options)
        -> Result<(Renderer, Device<P, device::Device>), InitError> {
    device::init(graphics_context, options).map(|(server, client)| {
        ((render::start(options, server), client))
    })
}
