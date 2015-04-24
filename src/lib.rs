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

#![feature(alloc, core)]

extern crate alloc;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
extern crate draw_state;
extern crate num;

/// public re-exported traits
pub mod traits {
    pub use device::{Device, Factory};
    pub use render::ext::canvas::IntoCanvas;
    pub use render::ext::device::DeviceExt;
    pub use render::ext::factory::{RenderFactory, FactoryExt};
    pub use render::mesh::ToSlice;
    pub use render::target::Output;
}

// draw state re-exports
pub use draw_state::{DrawState, BlendPreset};

// public re-exports
pub use render::{Renderer, DrawError};
pub use render::batch;
pub use render::ext::canvas::{Canvas, Window};
pub use render::ext::device::Graphics;
pub use render::ext::shade::{ShaderSource, ProgramError};
pub use render::mesh::{Attribute, Mesh, VertexFormat};
pub use render::mesh::Error as MeshError;
pub use render::mesh::{Slice, ToSlice, SliceKind};
pub use render::shade;
pub use render::target::{Frame, Output, Plane};
pub use render::ParamStorage;
pub use device::{Device, SubmitInfo, Factory, Resources};
pub use device::{attrib, tex};
pub use device::as_byte_slice;
pub use device::{BufferRole, BufferInfo, BufferUsage};
pub use device::{VertexCount, InstanceCount};
pub use device::PrimitiveType;
pub use device::draw::{CommandBuffer, Gamma};
pub use device::shade::{ProgramInfo, UniformValue};
pub use draw_state::target::*;
pub use draw_state::state;

pub use device::handle::Buffer as BufferHandle;
pub use device::handle::IndexBuffer as IndexBufferHandle;
pub use device::handle::RawBuffer as RawBufferHandle;
pub use device::handle::Shader as ShaderHandle;
pub use device::handle::Program as ProgramHandle;
pub use device::handle::FrameBuffer as FrameBufferHandle;
pub use device::handle::Surface as SurfaceHandle;
pub use device::handle::Texture as TextureHandle;
pub use device::handle::Sampler as SamplerHandle;

pub mod render;
pub mod device;
