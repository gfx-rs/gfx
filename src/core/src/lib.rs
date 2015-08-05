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

//! An efficient, low-level, bindless graphics API for Rust. See [the
//! blog](http://gfx-rs.github.io/) for explanations and annotated examples.

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
extern crate draw_state;
extern crate num;

/// public re-exported traits
pub mod traits {
    pub use device::{Device, Factory, DeviceFence};
    pub use extra::factory::FactoryExt;
    pub use extra::stream::{Stream, StreamFactory};
    pub use render::RenderFactory;
    pub use render::mesh::{ToIndexSlice, ToSlice};
    pub use render::target::Output;
}

// draw state re-exports
pub use draw_state::{DrawState, BlendPreset};
pub use draw_state::state;
pub use draw_state::target::*;

// public re-exports
pub use device::{Device, SubmitInfo, Factory, Resources};
pub use device::{attrib, tex, handle};
pub use device::as_byte_slice;
pub use device::{BufferRole, BufferInfo, BufferUsage};
pub use device::{VertexCount, InstanceCount};
pub use device::PrimitiveType;
pub use device::draw::{CommandBuffer, Gamma, InstanceOption};
pub use device::shade::{ProgramInfo, UniformValue};
pub use render::{Renderer, BlitError, DrawError, UpdateError};
pub use render::batch;
pub use render::mesh::{Attribute, Mesh, VertexFormat};
pub use render::mesh::Error as MeshError;
pub use render::mesh::{Slice, ToIndexSlice, ToSlice, SliceKind};
pub use render::shade;
pub use render::target::{Frame, Output, Plane};
pub use render::ParamStorage;
pub use extra::shade::{ShaderSource, ProgramError};
pub use extra::stream::{OwnedStream, Stream, Window};

pub mod device;
pub mod extra;
pub mod macros;
pub mod render;
