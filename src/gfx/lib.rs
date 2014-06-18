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

pub use Renderer = render::Client;
pub use Platform = device::Server;
pub use device::InitError;

mod server;
mod device;
mod render;

pub fn start(options: ()) -> Result<(Renderer, Platform), InitError> {
    device::init(options).map(|(device, platform)| {
        ((render::start(options, device), platform))
    })
}
