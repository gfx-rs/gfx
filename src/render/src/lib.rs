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

#[cfg(feature = "cgmath-types")]
extern crate cgmath;

extern crate log;
#[macro_use]
extern crate derivative;
extern crate draw_state;
extern crate gfx_core as core;

/// public re-exported traits
pub mod traits {
    pub use core::{Device, Factory};
    pub use core::memory::Pod;
    pub use factory::FactoryExt;
}

// draw state re-exports
pub use draw_state::{preset, state};
pub use draw_state::target::*;

// public re-exports
pub use core::{Device, Primitive, Resources, SubmissionError, SubmissionResult};
pub use core::{VertexCount, InstanceCount};
pub use core::{ShaderSet, VertexShader, HullShader, DomainShader, GeometryShader, PixelShader};
pub use core::{buffer, format, handle, texture, mapping};
pub use core::factory::{Factory, ResourceViewError, TargetViewError, CombinedError};
pub use core::memory::{self, Bind, TRANSFER_SRC, TRANSFER_DST, RENDER_TARGET,
                       DEPTH_STENCIL, SHADER_RESOURCE, UNORDERED_ACCESS};
pub use core::command::{Buffer as CommandBuffer, InstanceParams};
pub use core::shade::{ProgramInfo, UniformValue};

pub use encoder::{CopyBufferResult, CopyBufferTextureResult, CopyError,
                  CopyTextureBufferResult, Encoder, UpdateError};
pub use factory::PipelineStateError;
pub use slice::{Slice, IntoIndexBuffer, IndexBuffer};
pub use pso::{PipelineState};
pub use pso::buffer::{VertexBuffer, InstanceBuffer, RawVertexBuffer,
                      ConstantBuffer, RawConstantBuffer, Global, RawGlobal};
pub use pso::resource::{ShaderResource, RawShaderResource, UnorderedAccess,
                        Sampler, TextureSampler};
pub use pso::target::{DepthStencilTarget, DepthTarget, StencilTarget,
                      RenderTarget, RawRenderTarget, BlendTarget, BlendRef, Scissor};
pub use pso::bundle::{Bundle};

/// Render commands encoder
mod encoder;
/// Factory extensions
mod factory;
/// Slices
mod slice;
// Pipeline states
pub mod pso;
/// Shaders
pub mod shade;
/// Convenience macros
pub mod macros;
