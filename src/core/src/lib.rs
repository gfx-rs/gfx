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

#[cfg(feature = "cgmath-types")]
extern crate cgmath;

#[cfg(feature = "serialize")]
extern crate serde;
#[cfg(feature = "serialize")]
#[macro_use]
extern crate serde_derive;

use std::fmt::{self, Debug};
use std::error::Error;
use std::hash::Hash;
use std::any::Any;
use std::borrow::Borrow;

pub use draw_state::{state, target};
pub use self::command::CommandBuffer;
pub use self::factory::Factory;
pub use self::queue::{
    GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};

pub mod buffer;
pub mod command;
pub mod dummy;
pub mod factory;
pub mod format;
pub mod handle;
pub mod mapping;
pub mod memory;
pub mod pool;
pub mod pso;
pub mod queue;
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

///
#[derive(Copy, Clone, Debug)]
pub struct HeapType {
    /// Id of the heap type.
    pub id: usize,
    /// Properties of the associated heap memory.
    pub properties: memory::HeapProperties,
    /// Index to the underlying memory heap.
    pub heap_index: usize,
}

/// Different types of a specific API.
#[allow(missing_docs)]
pub trait Backend: Sized {
    type Resources: Resources;
    type CommandQueue: CommandQueue<Self>;
    type GeneralCommandBuffer: CommandBuffer<Self> + command::Buffer<Self::Resources>; // + GraphicsCommandBuffer<Self::R> + ComputeCommandBuffer<Self::R>;
    type GraphicsCommandBuffer: CommandBuffer<Self> + command::Buffer<Self::Resources>; // + GraphicsCommandBuffer<Self::R>;
    type ComputeCommandBuffer: CommandBuffer<Self>; // + ComputeCommandBuffer<Self::R>;
    type TransferCommandBuffer: CommandBuffer<Self>; // + TransferCommandBuffer<Self::R>;
    type SubpassCommandBuffer: CommandBuffer<Self>; // + SubpassCommandBuffer<Self::R>;
    type SubmitInfo;
    type Factory: Factory<Self::Resources>;
    type QueueFamily: QueueFamily;
}

/// Different resource types of a specific API.
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
    type Fence:               Debug + Hash + Eq + PartialEq + Any + Send + Sync;
    type Semaphore:           Debug + Any + Send + Sync;
    type Mapping:             Hash + Debug + Eq + PartialEq + Any + Send + Sync + mapping::Gate<Self>;
}

/*
/// Different resource types of a specific API. 
pub trait Resources:          Clone + Hash + Debug + Any {
    type ShaderLib:           Debug + Any + Send + Sync;
    type RenderPass:          Debug + Any + Send + Sync;
    type PipelineLayout:      Debug + Any + Send + Sync;
    type GraphicsPipeline:    Debug + Any + Send + Sync;
    type ComputePipeline:     Debug + Any + Send + Sync;
    type UnboundBuffer:       Debug + Any + Send + Sync;
    type Buffer:              Debug + Any + Send + Sync;
    type UnboundImage:        Debug + Any + Send + Sync;
    type Image:               Debug + Any + Send + Sync;
    type ConstantBufferView:  Debug + Any + Send + Sync;
    type ShaderResourceView:  Debug + Any + Send + Sync;
    type UnorderedAccessView: Debug + Any + Send + Sync;
    type RenderTargetView:    Debug + Any + Send + Sync;
    type DepthStencilView:    Debug + Any + Send + Sync;
    type FrameBuffer:         Debug + Any + Send + Sync;
    type Sampler:             Debug + Any + Send + Sync;
    type Semaphore:           Debug + Any + Send + Sync;
    type Fence:               Debug + Any + Send + Sync;
    type Heap:                Debug + Any;
    type Mapping;
    type DescriptorHeap:      Debug + Any;
    type DescriptorSetPool:   Debug + Any;
    type DescriptorSet:       Debug + Any;
    type DescriptorSetLayout: Debug + Any;
}
*/

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

/// 
pub struct Device_<B: Backend> {
    /// Resource factory.
    pub factory: B::Factory,
    /// General command queues.
    pub general_queues: Vec<GeneralQueue<B>>,
    /// Graphics command queues.
    pub graphics_queues: Vec<GraphicsQueue<B>>,
    /// Compute command queues.
    pub compute_queues: Vec<ComputeQueue<B>>,
    /// Transfer command queues.
    pub transfer_queues: Vec<TransferQueue<B>>,
    /// Types of memory heaps.
    pub heap_types: Vec<HeapType>,
    /// Memory heaps.
    pub memory_heaps: Vec<u64>,

    ///
    pub _marker: std::marker::PhantomData<B>
}

/// Represents a physical or virtual device, which is capable of running the backend.
pub trait Adapter<B: Backend>: Sized {
    /// Create a new device and command queues.
    fn open(&self, queue_descs: &[(&B::QueueFamily, u32)]) -> Device_<B>;

    /// Get the `AdapterInfo` for this adapater.
    fn get_info(&self) -> &AdapterInfo;

    /// Return the supported queue families for this adapter.
    fn get_queue_families(&self) -> &[B::QueueFamily];
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
    /// Return the number of available queues of this family
    // TODO: some backends like d3d12 support infinite software queues (verify)
    fn num_queues(&self) -> u32;
}

/// Submission information for a command queue.
pub struct QueueSubmit<'a, B: Backend + 'a> {
    /// Command buffers to submit.
    pub cmd_buffers: &'a [command::Submit<B>],
    /// Semaphores to wait being signaled before submission.
    pub wait_semaphores: &'a [(&'a mut <B::Resources as Resources>::Semaphore, pso::PipelineStage)],
    /// Semaphores which get signaled after submission.
    pub signal_semaphores: &'a [&'a mut <B::Resources as Resources>::Semaphore],
}

/// Dummy trait for command queues.
/// CommandBuffers will be later submitted to command queues instead of the device.
pub trait CommandQueue<B: Backend> {
    /// Submit command buffers to queue for execution.
    unsafe fn submit(&mut self, submit_infos: &[QueueSubmit<B>], fence: Option<&mut <B::Resources as Resources>::Fence>);
    
    ///
    fn wait_idle(&mut self);
}

/// `CommandPool` can allocate command buffers of a specific type only.
/// The allocated command buffers are associated with the creating command queue.
pub trait CommandPool<B: Backend> {
    /// Reset the command pool and the corresponding command buffers.
    ///
    /// # Synchronization: You may _not_ free the pool if a command buffer is still in use (pool memory still in use)
    fn reset(&mut self);

    /// Reserve an additional amount of command buffers.
    fn reserve(&mut self, additional: usize);
}

/// A `Surface` abstracts the surface of a native window, which will be presented
pub trait Surface<B: Backend> {
    ///
    type SwapChain: SwapChain<B>;

    /// Check if the queue family supports presentation for this surface.
    fn supports_queue(&self, queue_family: &B::QueueFamily) -> bool;

    /// Create a new swapchain from the current surface with an associated present queue.
    fn build_swapchain<Cf, Q>(&self, present_queue: Q) -> Self::SwapChain
        where Cf: format::RenderFormat,
              Q: Borrow<B::CommandQueue>;
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

/// Synchronization primitives which will be signaled once a frame got retrieved.
///
/// The semaphore or fence _must_ be unsignaled.
pub enum FrameSync<'a, R: Resources> {
    /// Semaphore used for synchronization.
    ///
    /// Will be signaled once the frame backbuffer is available.
    Semaphore(&'a R::Semaphore),

    /// Fence used for synchronization.
    ///
    /// Will be signaled once the frame backbuffer is available.
    Fence(&'a R::Fence)
}

/// The `SwapChain` is the backend representation of the surface.
/// It consists of multiple buffers, which will be presented on the surface.
pub trait SwapChain<B: Backend> {
    /// Access the backbuffer images.
    fn get_images(&mut self) -> &[handle::RawTexture<B::Resources>];

    /// Acquire a new frame for rendering. This needs to be called before presenting.
    fn acquire_frame(&mut self, sync: FrameSync<B::Resources>) -> Frame;

    /// Present one acquired frame in FIFO order.
    fn present(&mut self);
}

/// Extension for windows.
/// Main entry point for backend initialization from a window.
pub trait WindowExt<B: Backend> {
    /// Associated `Surface` type.
    type Surface: Surface<B>;
    /// Associated `Adapter` type.
    type Adapter: Adapter<B>;

    /// Create window surface and enumerate all available adapters.
    fn get_surface_and_adapters(&mut self) -> (Self::Surface, Vec<Self::Adapter>);
}
