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
extern crate derivative;
extern crate draw_state;
extern crate log;

#[cfg(feature = "mint")]
extern crate mint;

#[cfg(feature = "serialize")]
extern crate serde;
#[cfg(feature = "serialize")]
#[macro_use]
extern crate serde_derive;

use std::fmt::{self, Debug};
use std::error::Error;
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
/// Number of vertices in a patch
pub type PatchSize = u8;

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
    ( $($name:ident),+ ) => {
        $(
        #[allow(missing_docs)]
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub struct $name<R: Resources>(handle::Shader<R>);

        impl<R: Resources> $name<R> {
            #[allow(missing_docs)]
            pub fn reference(&self, man: &mut handle::Manager<R>) -> &R::Shader {
                man.ref_shader(&self.0)
            }

            #[doc(hidden)]
            pub fn new(shader: handle::Shader<R>) -> Self {
                $name(shader)
            }
        }
        )+
    }
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

//TODO: use the appropriate units for max vertex count, etc
/// Features that the device supports.
#[allow(missing_docs)] // pretty self-explanatory fields!
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Capabilities {
    pub max_vertex_count: usize,
    pub max_index_count: usize,
    pub max_texture_size: usize,
    pub max_patch_size: usize,

    pub instance_base_supported: bool,
    pub instance_call_supported: bool,
    pub instance_rate_supported: bool,
    pub vertex_base_supported: bool,
    pub srgb_color_supported: bool,
    pub constant_buffer_supported: bool,
    pub unordered_access_view_supported: bool,
    pub separate_blending_slots_supported: bool,
    pub copy_buffer_supported: bool,
}

/// Describes what geometric primitives are created from vertex data.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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
    /// Each quadtruplet of vertices represent a single line segment with adjacency information.
    /// For example, with `[a, b, c, d]`, `b` and `c` form a line, and `a` and `d` are the adjacent
    /// vertices.
    LineListAdjacency,
    /// Every four consecutive vertices represent a single line segment with adjacency information.
    /// For example, with `[a, b, c, d, e]`, `[a, b, c, d]` form a line segment with adjacency, and
    /// `[b, c, d, e]` form a line segment with adjacency.
    LineStripAdjacency,
    /// Each sextuplet of vertices represent a single traingle with adjacency information. For
    /// example, with `[a, b, c, d, e, f]`, `a`, `c`, and `e` form a traingle, and `b`, `d`, and
    /// `f` are the adjacent vertices, where `b` is adjacent to the edge formed by `a` and `c`, `d`
    /// is adjacent to the edge `c` and `e`, and `f` is adjacent to the edge `e` and `a`.
    TriangleListAdjacency,
    /// Every even-numbered vertex (every other starting from the first) represents an additional
    /// vertex for the triangle strip, while odd-numbered vertices (every other starting from the
    /// second) represent adjacent vertices. For example, with `[a, b, c, d, e, f, g, h]`, `[a, c,
    /// e, g]` form a triangle strip, and `[b, d, f, h]` are the adjacent vertices, where `b`, `d`,
    /// and `f` are adjacent to the first triangle in the strip, and `d`, `f`, and `h` are adjacent
    /// to the second.
    TriangleStripAdjacency,
    /// Patch list,
    /// used with shaders capable of producing primitives on their own (tessellation)
    PatchList(PatchSize),
}

/// A type of each index value in the slice's index buffer
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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
    type Fence:               Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;
    type Mapping:             Hash + Debug + Eq + PartialEq + Any + Send + Sync + mapping::Gate<Self>;
}

#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum SubmissionError {
    AccessOverlap,
}

impl fmt::Display for SubmissionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::SubmissionError::*;
        match *self {
            AccessOverlap => write!(f, "{}", self.description()),
        }
    }
}

impl Error for SubmissionError {
    fn description(&self) -> &str {
        use self::SubmissionError::*;
        match *self {
            AccessOverlap => "A resource access overlaps with another"
        }
    }
}

#[allow(missing_docs)]
pub type SubmissionResult<T> = Result<T, SubmissionError>;

/// A `Device` is responsible for submitting `CommandBuffer`s to the GPU.
pub trait Device: Sized {
    /// Associated `Resources` type.
    type Resources: Resources;
    /// Associated `CommandBuffer` type. Every `Device` type can only work with one `CommandBuffer`
    /// type.
    type CommandBuffer: command::Buffer<Self::Resources>;

    /// Returns the capabilities of this `Device`.
    fn get_capabilities(&self) -> &Capabilities;

    /// Pin everything from this handle manager to live for a frame.
    fn pin_submitted_resources(&mut self, &handle::Manager<Self::Resources>);

    /// Submits a `CommandBuffer` to the GPU for execution.
    fn submit(&mut self,
              &mut Self::CommandBuffer,
              access: &command::AccessInfo<Self::Resources>)
              -> SubmissionResult<()>;

    /// Submits a `CommandBuffer` to the GPU for execution.
    /// returns a fence that is signaled after the GPU has executed all commands
    fn fenced_submit(&mut self,
                     &mut Self::CommandBuffer,
                     access: &command::AccessInfo<Self::Resources>,
                     after: Option<handle::Fence<Self::Resources>>)
                     -> SubmissionResult<handle::Fence<Self::Resources>>;

    /// Stalls the current thread until the fence is satisfied
    fn wait_fence(&mut self, &handle::Fence<Self::Resources>);

    /// Cleanup unused resources. This should be called between frames.
    fn cleanup(&mut self);
}

/// Represents a physical or virtual device, which is capable of running the backend.
pub trait Adapter: Sized {
    /// Associated `CommandQueue` type.
    type CommandQueue: CommandQueue;
    /// Associated `Device` type.
    type Device: Device;
    /// Associated `QueueFamily` type.
    type QueueFamily: QueueFamily;

    /// Enumerate all available adapters supporting this backend
    fn enumerate_adapters() -> Vec<Self>;

    /// Create a new device and command queues.
    fn open<'a, I>(&self, queue_descs: I) -> (Self::Device, Vec<Self::CommandQueue>)
        where I: Iterator<Item=(&'a Self::QueueFamily, u32)>;

    /// Get the `AdapterInfo` for this adapater.
    fn get_info(&self) -> &AdapterInfo;

    /// Return the supported queue families for this adapter.
    fn get_queue_families(&self) -> &[Self::QueueFamily];
}

/// Information about a backend adapater.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct AdapterInfo {
    /// Adapter name
    pub name: String,
    /// Vendor PCI id of the adapter
    pub vendor: usize,
    /// PCI id of the adapter
    pub device: usize,
    /// The device is based on a software rasterizer
    pub software_rendering: bool,
}

/// `QueueFamily` denotes a group of command queues provided by the backend
/// with the same properties/type.
pub trait QueueFamily: 'static {
    /// Associated `Surface` type.
    type Surface: Surface;

    /// Check if the queue family supports presentation to a surface
    fn supports_present(&self, surface: &Self::Surface) -> bool;

    /// Return the number of available queues of this family
    // TODO: some backends like d3d12 support infinite software queues (verify)
    fn num_queues(&self) -> u32;
}

/// Dummy trait for command queues.
/// CommandBuffers will be later submitted to command queues instead of the device.
pub trait CommandQueue { }

/// A `Surface` abstracts the surface of a native window, which will be presented
pub trait Surface {
    /// Associated `CommandQueue` type.
    type CommandQueue: CommandQueue;
    /// Associated `SwapChain` type.
    type SwapChain: SwapChain;
    /// Associated native `Window` type.
    type Window;

    /// Create a new surface from a native window.
    fn from_window(window: &Self::Window) -> Self;

    /// Create a new swapchain from the current surface with an associated present queue.
    fn build_swapchain<T: format::RenderFormat>(&self, present_queue: &Self::CommandQueue)
        -> Self::SwapChain;
}

/// Handle to a backbuffer of the swapchain.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Frame(usize);

impl Frame {
    #[doc(hidden)]
    pub fn new(id: usize) -> Self {
        Frame(id)
    }
}

/// The `SwapChain` is the backend representation of the surface.
/// It consists of multiple buffers, which will be presented on the surface.
pub trait SwapChain {
    /// Acquire a new frame for rendering. This needs to be called before presenting.
    fn acquire_frame(&mut self) -> Frame;

    /// Present one acquired frame in FIFO order.
    fn present(&mut self);
}
