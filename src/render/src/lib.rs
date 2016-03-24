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

#![deny(missing_docs)]

//! An efficient, low-level, bindless graphics API for Rust. See [the
//! blog](http://gfx-rs.github.io/) for explanations and annotated examples.

#[macro_use]
extern crate log;
extern crate draw_state;
extern crate gfx_core;

/// public re-exported traits
pub mod traits {
    pub use gfx_core::{Device, Factory, DeviceFence};
    pub use factory::FactoryExt;
}

// draw state re-exports
pub use draw_state::{preset, state};
pub use draw_state::target::*;

// public re-exports
pub use gfx_core as core;
pub use gfx_core::{Device, Resources, Primitive};
pub use gfx_core::{VertexCount, InstanceCount};
pub use gfx_core::{ShaderSet, VertexShader, HullShader, DomainShader,
                   GeometryShader, PixelShader};
pub use gfx_core::{format, handle, tex};
pub use gfx_core::factory::{Factory, Usage, Bind, MapAccess, ResourceViewError, TargetViewError,
                            BufferRole, BufferInfo, BufferError, BufferUpdateError, CombinedError,
                            RENDER_TARGET, DEPTH_STENCIL, SHADER_RESOURCE, UNORDERED_ACCESS,
                            cast_slice};
pub use gfx_core::draw::{CommandBuffer, InstanceOption};
pub use gfx_core::shade::{ProgramInfo, UniformValue};

pub use encoder::{Encoder, UpdateError};
pub use factory::PipelineStateError;
pub use mesh::{Slice, ToIndexSlice, SliceKind};
pub use pso::{PipelineState};
pub use pso::buffer::{VertexBuffer, InstanceBuffer, RawVertexBuffer,
                      ConstantBuffer, Global};
pub use pso::resource::{ShaderResource, RawShaderResource, UnorderedAccess,
                        Sampler, TextureSampler};
pub use pso::target::{DepthStencilTarget, DepthTarget, StencilTarget,
                      RenderTarget, RawRenderTarget, BlendTarget, BlendRef, Scissor};

/// Render commands encoder
mod encoder;
/// Factory extensions
mod factory;
/// Meshes
mod mesh;
/// Pipeline states
pub mod pso;
/// Shaders
pub mod shade;
/// Convenience macros
pub mod macros;
