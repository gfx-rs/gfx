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

#![feature(phase)]
#[phase(plugin, link)] extern crate log;
extern crate libc;

pub use Renderer = render::Client;
pub use MeshSlice = render::mesh::Slice;
pub use VertexSlice = render::mesh::VertexSlice;
pub use Device = device::Server;
pub use device::InitError;
pub use platform::GraphicsContext;
pub use render::target::ClearData;

pub type Options<'a> = &'a platform::GlProvider;

mod device;
mod render;
pub mod platform;

pub fn start<Api, P: GraphicsContext<Api>>(graphics_context: P, options: Options)
        -> Result<(Renderer, Device<P>), InitError> {
    device::init(graphics_context, options).map(|(server, client)| {
        ((render::start(options, server), client))
    })
}
