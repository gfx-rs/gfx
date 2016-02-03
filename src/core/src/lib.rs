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

//! Low-level graphics abstraction for Rust. Mostly operates on data, not types.
//! Designed for use by libraries and higher-level abstractions only.

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
extern crate draw_state;
//extern crate num;

use std::fmt::Debug;
use std::hash::Hash;

pub use draw_state::{state, target};
pub use self::factory::Factory;

pub mod draw;
pub mod dummy;
pub mod factory;
pub mod format;
pub mod handle;
pub mod mapping;
pub mod pso;
pub mod shade;
pub mod tex;

/// Compile-time maximum number of vertex attributes.
pub const MAX_VERTEX_ATTRIBUTES: usize = 16;
/// Compile-time maximum number of color targets.
pub const MAX_COLOR_TARGETS:      usize = 4;
/// Compile-time maximum number of constant buffers.
pub const MAX_CONSTANT_BUFFERS: usize = 16;
/// Compile-time maximum number of shader resource views (SRV).
pub const MAX_RESOURCE_VIEWS: usize = 32;
/// Compile-time maximum number of unordered access views (UAV).
pub const MAX_UNORDERED_VIEWS: usize = 4;
/// Compile-time maximum number of samplers.
pub const MAX_SAMPLERS: usize = 16;

/// Draw vertex count.
pub type VertexCount = u32;
/// Draw number of instances
pub type InstanceCount = u32;

/// Slot for an attribute.
pub type AttributeSlot = u8;
/// Slot for a constant buffer object.
pub type ConstantBufferSlot = u8;
/// Slot for a shader resource view.
pub type ResourceViewSlot = u8;
/// Slot for an unordered access object.
pub type UnorderedViewSlot = u8;
/// Slot for an active color buffer.
pub type ColorSlot = u8;
/// Slot for a sampler.
pub type SamplerSlot = u8;

macro_rules! define_shaders {
    ($($name:ident),+) => {$(
        #[allow(missing_docs)]
        pub struct $name<R: Resources>(handle::Shader<R>);
        impl<R: Resources> $name<R> {
            #[allow(missing_docs)]
            pub fn reference(&self, man: &mut handle::Manager<R>) -> &R::Shader {
                man.ref_shader(&self.0)
            }
        }
    )+}
}

define_shaders!(VertexShader, HullShader, DomainShader, GeometryShader, PixelShader);

/// A complete set of shaders to link a program.
pub enum ShaderSet<R: Resources> {
    /// Simple program: Vs-Ps
    Simple(VertexShader<R>, PixelShader<R>),
    /// Geometry shader programs: Vs-Gs-Ps
    Geometry(VertexShader<R>, GeometryShader<R>, PixelShader<R>),
    //TODO: Tessellated, TessellatedGeometry, TransformFeedback
}

/// Features that the device supports.
#[derive(Copy, Clone, Debug)]
#[allow(missing_docs)] // pretty self-explanatory fields!
pub struct Capabilities {
    pub shader_model: shade::ShaderModel,

    pub max_vertex_count: usize,
    pub max_index_count: usize,
    pub max_draw_buffers: usize,
    pub max_texture_size: usize,
    pub max_vertex_attributes: usize,

    /// In GLES it is not allowed to re-bind a buffer to a different
    /// target than the one it was initialized with.
    pub buffer_role_change_allowed: bool,

    pub array_buffer_supported: bool,
    pub fragment_output_supported: bool,
    pub immutable_storage_supported: bool,
    pub instance_base_supported: bool,
    pub instance_call_supported: bool,
    pub instance_rate_supported: bool,
    pub render_targets_supported: bool,
    pub sampler_objects_supported: bool,
    pub srgb_color_supported: bool,
    pub uniform_block_supported: bool,
    pub vertex_base_supported: bool,
    pub separate_blending_slots_supported: bool,
}

/// Describes what geometric primitives are created from vertex data.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum Primitive {
    /// Each vertex represents a single point.
    PointList,
    /// Each pair of vertices represent a single line segment. For example, with `[a, b, c, d,
    /// e]`, `a` and `b` form a line, `c` and `d` form a line, and `e` is discarded.
    LineList,
    /// Every two consecutive vertices represent a single line segment. Visually forms a "path" of
    /// lines, as they are all connected. For example, with `[a, b, c]`, `a` and `b` form a line
    /// line, and `b` and `c` form a line.
    LineStrip,
    /// Each triplet of vertices represent a single triangle. For example, with `[a, b, c, d, e]`,
    /// `a`, `b`, and `c` form a triangle, `d` and `e` are discarded.
    TriangleList,
    /// Every three consecutive vertices represent a single triangle. For example, with `[a, b, c,
    /// d]`, `a`, `b`, and `c` form a triangle, and `b`, `c`, and `d` form a triangle.
    TriangleStrip,
    //Quad,
}

/// A type of each index value in the mesh's index buffer
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[allow(missing_docs)]
#[repr(u8)]
pub enum IndexType {
    U8,
    U16,
    U32,
}

/// Resources pertaining to a specific API.
#[allow(missing_docs)]
pub trait Resources:          Clone + Hash + Debug + Eq + PartialEq {
    type Buffer:              Clone + Hash + Debug + Eq + PartialEq + Send + Copy;
    type Shader:              Clone + Hash + Debug + Eq + PartialEq + Send;
    type Program:             Clone + Hash + Debug + Eq + PartialEq + Send;
    type PipelineStateObject: Clone + Hash + Debug + Eq + PartialEq + Send;
    type Texture:             Clone + Hash + Debug + Eq + PartialEq + Send;
    type ShaderResourceView:  Clone + Hash + Debug + Eq + PartialEq + Send + Copy;
    type UnorderedAccessView: Clone + Hash + Debug + Eq + PartialEq + Send + Copy;
    type RenderTargetView:    Clone + Hash + Debug + Eq + PartialEq + Send + Copy;
    type DepthStencilView:    Clone + Hash + Debug + Eq + PartialEq + Send;
    type Sampler:             Clone + Hash + Debug + Eq + PartialEq + Send + Copy;
    type Fence:               Clone + Hash + Debug + Eq + PartialEq + Send;
}

/// All the data needed simultaneously for submitting a command buffer for
/// execution on a device.
pub struct SubmitInfo<'a, D>(
    pub &'a D::CommandBuffer,
    pub &'a draw::DataBuffer,
    pub &'a handle::Manager<D::Resources>
) where
    D: Device,
    D::CommandBuffer: 'a,
    D::Resources: 'a;

/// An interface for performing draw calls using a specific graphics API
pub trait Device: Sized {
    /// Associated resources type.
    type Resources: Resources;
    /// Associated command buffer type.
    type CommandBuffer: draw::CommandBuffer<Self::Resources>;

    /// Returns the capabilities available to the specific API implementation.
    fn get_capabilities<'a>(&'a self) -> &'a Capabilities;

    /// Reset all the states to disabled/default.
    fn reset_state(&mut self);

    /// Submit a command buffer for execution.
    fn submit(&mut self, SubmitInfo<Self>);

    /// Cleanup unused resources, to be called between frames.
    fn cleanup(&mut self);
}

/// Extension to the Device that allows for submitting of commands
/// around a fence
pub trait DeviceFence<R: Resources>: Device<Resources=R> where
    <Self as Device>::CommandBuffer: draw::CommandBuffer<R> {
    /// Submit a command buffer to the stream creating a fence
    /// the fence is signaled after the GPU has executed all commands
    /// in the buffer
    fn fenced_submit(&mut self, SubmitInfo<Self>, after: Option<handle::Fence<R>>) -> handle::Fence<R>;

    /// Wait on the supplied fence stalling the current thread until
    /// the fence is satisfied
    fn fence_wait(&mut self, fence: &handle::Fence<R>);
}
