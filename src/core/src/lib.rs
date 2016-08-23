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
use std::any::Any;

pub use draw_state::{state, target};
pub use self::factory::Factory;

pub mod buffer;
pub mod command;
pub mod dummy;
pub mod factory;
pub mod format;
pub mod handle;
pub mod mapping;
pub mod memory;
pub mod pso;
pub mod shade;
pub mod texture;

/// Compile-time maximum number of vertex attributes.
pub const MAX_VERTEX_ATTRIBUTES: usize = 16;
/// Compile-time maximum number of color targets.
pub const MAX_COLOR_TARGETS: usize = 4;
/// Compile-time maximum number of constant buffers.
pub const MAX_CONSTANT_BUFFERS: usize = 14;
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
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ShaderSet<R: Resources> {
    /// Simple program: Vs-Ps
    Simple(VertexShader<R>, PixelShader<R>),
    /// Geometry shader programs: Vs-Gs-Ps
    Geometry(VertexShader<R>, GeometryShader<R>, PixelShader<R>),
    /// Tessellated TODO: Tessellated, TessellatedGeometry, TransformFeedback
    Tessellated(VertexShader<R>, HullShader<R>, DomainShader<R>, PixelShader<R>),

}

impl<R: Resources> ShaderSet<R> {
    /// Return the aggregated stage usage for the set.
    pub fn get_usage(&self) -> shade::Usage {
        match self {
            &ShaderSet::Simple(..) => shade::VERTEX | shade::PIXEL,
            &ShaderSet::Geometry(..) => shade::VERTEX | shade::GEOMETRY | shade::PIXEL,
            &ShaderSet::Tessellated(..) => shade::VERTEX | shade::HULL | shade::DOMAIN | shade::PIXEL,
        }
    }
}

/// Features that the device supports.
#[derive(Copy, Clone, Debug)]
#[allow(missing_docs)] // pretty self-explanatory fields!
pub struct Capabilities {
    pub max_vertex_count: usize,
    pub max_index_count: usize,
    pub max_texture_size: usize,

    pub instance_base_supported: bool,
    pub instance_call_supported: bool,
    pub instance_rate_supported: bool,
    pub vertex_base_supported: bool,
    pub srgb_color_supported: bool,
    pub constant_buffer_supported: bool,
    pub unordered_access_view_supported: bool,
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
    /// Quad,
    /// 4 point quad patch
    QuadList
}

/// A type of each index value in the slice's index buffer
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[allow(missing_docs)]
#[repr(u8)]
pub enum IndexType {
    U16,
    U32,
}

/// Different types of a specific API. 
#[allow(missing_docs)]
pub trait Resources:          Clone + Hash + Debug + Eq + PartialEq + Any {
    type Buffer:              Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
    type Shader:              Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type Program:             Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type PipelineStateObject: Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type Texture:             Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type ShaderResourceView:  Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
    type UnorderedAccessView: Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
    type RenderTargetView:    Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
    type DepthStencilView:    Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type Sampler:             Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync + Copy;
    type Fence:               Clone + Hash + Debug + Eq + PartialEq + Any + Fence;
    type Mapping:             Debug + Any + mapping::Gate<Self>;
}

/// A `Device` is responsible for submitting `CommandBuffer`s to the GPU. 
pub trait Device: Sized {
    /// Associated `Resources` type.
    type Resources: Resources;
    /// Associated `CommandBuffer` type. Every `Device` type can only work with one `CommandBuffer`
    /// type.
    type CommandBuffer: command::Buffer<Self::Resources>;

    /// Returns the capabilities of this `Ðevice`.
    fn get_capabilities(&self) -> &Capabilities;

    /// Pin everything from this handle manager to live for a frame.
    fn pin_submitted_resources(&mut self, &handle::Manager<Self::Resources>);

    /// Submits a `CommandBuffer` to the GPU for execution.

    fn submit(&mut self, &mut Self::CommandBuffer,
                         access: &pso::AccessInfo<Self::Resources>);

    /// Submits a `CommandBuffer` to the GPU for execution.
    /// returns a fence that is signaled after the GPU has executed all commands
    fn fenced_submit(&mut self,
                     &mut Self::CommandBuffer,
                     access: &pso::AccessInfo<Self::Resources>,
                     after: Option<handle::Fence<Self::Resources>>)
                     -> handle::Fence<Self::Resources>;

    /// Cleanup unused resources. This should be called between frames. 
    fn cleanup(&mut self);
}

/// Operations that must be provided by a fence.
pub trait Fence {
    /// Stalls the current thread until the fence is satisfied
    fn wait(&self);
}
