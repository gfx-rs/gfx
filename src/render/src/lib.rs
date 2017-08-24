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

// TODO(doc) clarify the different type of queues and what is accessible from the high-level API
// vs what belongs to core-ll. There doesn't seem to be a "ComputeEncoder" can I submit something
// built with a GraphicsEncoder to a ComputeQueue?

//! # gfx
//!
//! An efficient, low-level, bindless graphics API for Rust.
//!
//! # Overview
//!
//! ## Command buffers and encoders and queues
//!
//! A command buffer is a serialized list of drawing and compute commands.
//! Unlike with vulkan, command buffers are not what you use to create commands, but only
//! the result of creating these commands. Gfx, borrowing metal's terminology, uses
//! encoders to build command buffers. This means that, in general, users of the gfx crate
//! don't manipulate command buffers directly much and interact mostly with graphics encoders.
//! In order to be executed, a command buffer is then submitted to a queue.
//!
//! Manipulating a `GraphicsEncoder` in gfx corresponds to interacting with:
//!
//! - a `VkCommandBuffer` in vulkan,
//! - a `MTLCommandEncoder` in metal,
//! - an `ID3D12GraphicsCommandList` in D3D12.
//!
//! OpenGL and earlier versions of D3D don't have an explicit notion of command buffers
//! encoders or queues (with the exception of draw indirect commands in late versions of OpenGL,
//! which can be seen as a GPU-side command buffer). They are managed implicitly by the driver.
//!
//! See:
//!
//! - The [`GraphicsEncoder` struct](struct.GraphicsEncoder.html).
//! - The [`CommandBuffer` trait](trait.CommandBuffer.html).
//! - The [`CommandQueue` struct](struct.CommandQueue.html).
//!
//! ## Devoce
//!
//! The device is what lets you allocate GPU resources such as buffers and textures.
//!
//! Each gfx backend provides its own device type which implements both:
//!
//! - The [`Device` trait](traits/trait.Device.html#overview).
//! - The [`DeviceExt` trait](traits/trait.DeviceExt.html).
//!
//! `gfx::Device` is roughly equivalent to:
//!
//! - `VkDevice` in vulkan,
//! - `ID3D11Device` in D3D11,
//! - `MTLDevice` in metal.
//!
//! OpenGL does not have a notion of device (resources are created directly off of the global
//! context). D3D11 has a DXGI factory but it is only used to interface with other processes
//! and the window manager, resources like textures are usually created using the device.
//!
//! ## Gpu
//!
//! The `Gpu` contains the `Device` and the `Queue`s.
//!
//! ## Pipeline state (PSO)
//!
//! See [the documentation of the gfx::pso module](pso/index.html).
//!
//! ## Memory management
//!
//! Handles internally use atomically reference counted pointers to deal with memory management.
//! GPU resources are not destroyed right away when all references to them are gone. Instead they
//! are destroyed the next time `cleanup` is called on the queue.
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

#[macro_use]
extern crate log;
#[macro_use]
extern crate derivative;
extern crate draw_state;
extern crate gfx_core as core;

/// public re-exported traits
pub mod traits {
    pub use core::{Device};
    pub use core::memory::Pod;
    pub use device::DeviceExt;
}

// draw state re-exports
pub use draw_state::{preset, state};
pub use draw_state::target::*;

// public re-exports
pub use core::{Adapter, Backend, CommandQueue, Gpu, Frame, FrameSync, Headless, Primitive, QueueFamily, QueueType,
               Resources, SubmissionError, SubmissionResult, Surface, Swapchain, SwapchainConfig, WindowExt};
pub use core::{VertexCount, InstanceCount};
pub use core::{ShaderSet, VertexShader, HullShader, DomainShader, GeometryShader, PixelShader};
pub use core::{GeneralCommandPool, GraphicsCommandPool, ComputeCommandPool, SubpassCommandPool};
pub use core::{buffer, format, handle, texture, mapping, queue};
pub use core::device::{Device, ResourceViewError, TargetViewError, CombinedError, WaitFor};
pub use core::memory::{self, Bind, TRANSFER_SRC, TRANSFER_DST, RENDER_TARGET,
                       DEPTH_STENCIL, SHADER_RESOURCE, UNORDERED_ACCESS};
pub use core::command::{Buffer as CommandBuffer, InstanceParams};
pub use core::shade::{ProgramInfo, UniformValue};

pub use encoder::{CopyBufferResult, CopyBufferTextureResult, CopyError,
                  CopyTextureBufferResult, GraphicsEncoder, UpdateError, GraphicsPoolExt};
pub use device::PipelineStateError;
pub use slice::{Slice, IntoIndexBuffer, IndexBuffer};
pub use swapchain::SwapchainExt;
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
/// Device extensions
mod device;
/// Slices
mod slice;
/// Swapchain extensions
mod swapchain;
// Pipeline states
pub mod pso;
/// Shaders
pub mod shade;
/// Convenience macros
pub mod macros;
