// Copyright 2015 The Gfx-rs Developers.
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

//#[macro_use]
//extern crate bitflags;
#[macro_use]
extern crate log;
extern crate draw_state;
extern crate gfx_core;
//extern crate num;

/// public re-exported traits
pub mod traits {
    pub use gfx_core::{Device, Factory, DeviceFence};
    pub use gfx_core::output::Output;
    pub use extra::factory::FactoryExt;
    pub use extra::stream::{Stream, StreamFactory};
}

// draw state re-exports
pub use draw_state::{DrawState, blend, state};
pub use draw_state::target::*;

// public re-exports
pub use gfx_core as core;
pub use gfx_core::{Device, SubmitInfo, Resources};
pub use gfx_core::{attrib, format, handle, tex};
pub use gfx_core::factory::{Factory, BufferRole, BufferInfo, BufferUsage,
                            SHADER_RESOURCE, UNORDERED_ACCESS, RENDER_TARGET,
                            cast_slice};
pub use gfx_core::{VertexCount, InstanceCount};
pub use gfx_core::Primitive;
pub use gfx_core::{ShaderSet, VertexShader, HullShader, DomainShader,
                   GeometryShader, PixelShader};
pub use gfx_core::draw::{CommandBuffer, Gamma, InstanceOption};
pub use gfx_core::output::{Output, Plane};
pub use gfx_core::shade::{ProgramInfo, UniformValue};
pub use encoder::{Encoder, BlitError, UpdateError};
pub use mesh::{Attribute, Mesh, VertexFormat};
pub use mesh::Error as MeshError;
pub use mesh::{Slice, ToIndexSlice, SliceKind};
pub use pso::{PipelineState, VertexBuffer, ConstantBuffer,
              Global, PER_VERTEX, PER_INSTANCE,
              ResourceView, UnorderedView, Sampler, TextureSampler,
              RenderTarget, BlendTarget,
              DepthStencilTarget, DepthTarget, StencilTarget};
pub use target::{Frame};
pub use extra::factory::PipelineStateError;
pub use extra::stream::{OwnedStream, Stream, Window};

/// Render commands encoder
pub mod encoder;
/// Meshes
pub mod mesh;
/// Pipeline states
pub mod pso;
/// Shaders
pub mod shade;
/// Render targets
pub mod target;
/// Extra core extensions
pub mod extra;
/// Convenience macros
pub mod macros;
