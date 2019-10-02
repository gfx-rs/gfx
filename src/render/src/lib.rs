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

//! # gfx
//!
//! An efficient, low-level, bindless graphics API for Rust.
//!
//! # Overview
//!
//! ## Command buffers and encoders
//!
//! A command buffer is a serialized list of drawing and compute commands.
//! Unlike with vulkan, command buffers are not what you use to create commands, but only
//! the result of creating these commands. Gfx, borrowing metal's terminology, uses
//! encoders to build command buffers. This means that, in general, users of the gfx crate
//! don't manipulate command buffers directly much and interact mostly with encoders.
//!
//! Manipulating an `Encoder` in gfx corresponds to interacting with:
//!
//! - a `VkCommandBuffer` in vulkan,
//! - a `MTLCommandEncoder` in metal,
//! - an `ID3D12GraphicsCommandList` in D3D12.
//!
//! OpenGL and earlier versions of D3D don't have an explicit notion of command buffers
//! or encoders (with the exception of draw indirect commands in late versions of OpenGL,
//! which can be seen as a GPU-side command buffer). They are managed implicitly by the driver.
//!
//! See:
//!
//! - The [`Encoder` struct documentation](struct.Encoder.html).
//! - The [`Command buffer` trait documentation](trait.CommandBuffer.html).
//!
//! ## Factory
//!
//! The factory is what lets you allocate GPU resources such as buffers and textures.
//!
//! Each gfx backend provides its own factory type which implements both:
//!
//! - The [`Factory` trait](traits/trait.Factory.html#overview).
//! - The [`FactoryExt` trait](traits/trait.FactoryExt.html).
//!
//! `gfx::Factory` is roughly equivalent to:
//!
//! - `VkDevice` in vulkan,
//! - `ID3D11Device` in D3D11,
//! - `MTLDevice` in metal.
//!
//! OpenGL does not have a notion of factory (resources are created directly off of the global
//! context). D3D11 has a DXGI factory but it is only used to interface with other processes
//! and the window manager, resources like textures are usually created using the device.
//!
//! ## Device
//!
//! See [the `gfx::Device` trait](trait.Device.html).
//!
//! ## Pipeline state (PSO)
//!
//! See [the documentation of the gfx::pso module](pso/index.html).
//!
//! ## Memory management
//!
//! Handles internally use atomically reference counted pointers to deal with memory management.
//! GPU resources are not destroyed right away when all references to them are gone. Instead they
//! are destroyed the next time [Device::cleanup](trait.Device.html#tymethod.cleanup) is called.
//!
//! # Examples
//!
//! See [the examples in the repository](https://github.com/gfx-rs/gfx/tree/master/examples).
//!
//! # Useful resources
//!
//!  - [Documentation for some of the technical terms](doc/terminology/index.html)
//! used in the API.
//!  - [Learning gfx](https://wiki.alopex.li/LearningGfx) tutorial.
//!  - See [the blog](http://gfx-rs.github.io/) for more explanations and annotated examples.
//!

#[cfg(feature = "mint")]
extern crate mint;

extern crate log;
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
pub use core::{buffer, format, handle, mapping, memory, texture};
pub use core::factory::{Factory, ResourceViewError, TargetViewError, CombinedError};
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
