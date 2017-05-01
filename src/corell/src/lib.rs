// Copyright 2017 The Gfx-rs Developers.
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

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
extern crate draw_state;

use std::fmt::Debug;
use std::hash::Hash;
use std::any::Any;
use std::slice::Iter;
use std::ops::{Deref};

pub use draw_state::{state, target};
pub use self::factory::Factory;
pub use queue::{GeneralQueue, GraphicsQueue, ComputeQueue, TransferQueue};
pub use pool::{GeneralCommandPool, GraphicsCommandPool};
pub use command::{CommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer,
    SubpassCommandBuffer, ProcessingCommandBuffer, PrimaryCommandBuffer, SecondaryCommandBuffer};

pub mod buffer;
pub mod command;
pub mod factory;
pub mod format;
pub mod image;
pub mod mapping;
pub mod memory;
pub mod pass;
pub mod pool;
pub mod pso;
pub mod queue;
pub mod shade;

/// Compile-time maximum number of color targets.
pub const MAX_COLOR_TARGETS: usize = 8; // Limited by D3D12

/// Draw vertex count.
pub type VertexCount = u32;
/// Draw number of instances.
pub type InstanceCount = u32;
/// Draw vertex base offset.
pub type VertexOffset = i32;
/// Number of vertices in a patch.
pub type PatchSize = u8;

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
    /// Patch list,
    /// used with shaders capable of producing primitives on their own (tessellation)
    PatchList(PatchSize),
}

/// A type of each index value in the slice's index buffer
#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Copy, Clone, Debug)]
#[allow(missing_docs)]
#[repr(u8)]
pub enum IndexType {
    U16,
    U32,
}

pub struct SubPass<'a, R: Resources> {
    pub index: usize,
    pub main_pass: &'a R::RenderPass,
}

/// An `Instance` holds per-application state for a specific backend
pub trait Instance {
    type Adapter: Adapter;
    type Surface: Surface;
    type Window;

    /// Instantiate a new `Instance`, this is our entry point for applications
    fn create() -> Self;

    /// Enumerate all available adapters supporting this backend 
    fn enumerate_adapters(&self) -> Vec<Self::Adapter>;

    /// Create a new surface from a native window.
    fn create_surface(&self, window: &Self::Window) -> Self::Surface;
}

#[derive(Copy, Clone, Debug)]
pub struct HeapType {
    pub id: usize,
    pub properties: memory::HeapProperties,
    pub heap_index: usize,
}

pub struct Device<R: Resources, F: Factory<R>, Q: CommandQueue> {
    pub factory: F,
    pub general_queues: Vec<GeneralQueue<Q>>,
    pub graphics_queues: Vec<GraphicsQueue<Q>>,
    pub compute_queues: Vec<ComputeQueue<Q>>,
    pub transfer_queues: Vec<TransferQueue<Q>>,
    pub heap_types: Vec<HeapType>,
    pub memory_heaps: Vec<u64>,

    pub _marker: std::marker::PhantomData<*const R>
}

/// Represents a physical or virtual device, which is capable of running the backend.
pub trait Adapter {
    type CommandQueue: CommandQueue;
    type Resources: Resources;
    type Factory: Factory<Self::Resources>;
    type QueueFamily: QueueFamily;

    /// Create a new device and command queues.
    fn open<'a, I>(&self, queue_descs: I) -> Device<Self::Resources, Self::Factory, Self::CommandQueue>
        where I: ExactSizeIterator<Item=(&'a Self::QueueFamily, u32)>;

    /// Get the `AdapterInfo` for this adapater.
    fn get_info(&self) -> &AdapterInfo;

    /// Return the supported queue families for this adapter.
    fn get_queue_families(&self) -> Iter<Self::QueueFamily>;
}

/// Information about a backend adapater.
#[derive(Clone, Debug)]
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
    type Surface: Surface;

    /// Check if the queue family supports presentation to a surface
    fn supports_present(&self, surface: &Self::Surface) -> bool;

    /// Return the number of available queues of this family
    // TODO: some backends like d3d12 support infinite software queues (verify)
    fn num_queues(&self) -> u32;
}

pub struct QueueSubmit<'a, C: CommandBuffer + 'a, R: Resources> {
    pub cmd_buffers: &'a [command::Submit<C>],
    pub wait_semaphores: &'a [(&'a mut R::Semaphore, pso::PipelineStage)],
    pub signal_semaphores: &'a [&'a mut R::Semaphore],
}

/// `CommandBuffers` are submitted to a `CommandQueue` and executed in-order of submission.
/// `CommandQueue`s may run in parallel and need to be explicitly synchronized.
pub trait CommandQueue {
    type R: Resources;
    type SubmitInfo;
    type GeneralCommandBuffer: CommandBuffer<SubmitInfo = Self::SubmitInfo> + GraphicsCommandBuffer<Self::R> + ComputeCommandBuffer<Self::R>;
    type GraphicsCommandBuffer: CommandBuffer<SubmitInfo = Self::SubmitInfo> + GraphicsCommandBuffer<Self::R>;
    type ComputeCommandBuffer: CommandBuffer<SubmitInfo = Self::SubmitInfo> + ComputeCommandBuffer<Self::R>;
    type TransferCommandBuffer: CommandBuffer<SubmitInfo = Self::SubmitInfo> + TransferCommandBuffer<Self::R>;
    type SubpassCommandBuffer: CommandBuffer<SubmitInfo = Self::SubmitInfo>; // + SubpassCommandBuffer<Self::R>;

    /// Submit command buffers to queue for execution.
    unsafe fn submit<'a, C>(&mut self, submit_infos: &[QueueSubmit<C, Self::R>], fence: Option<&'a mut <Self::R as Resources>::Fence>)
        where C: CommandBuffer<SubmitInfo = Self::SubmitInfo>;

    ///
    fn wait_idle(&mut self);
}

/// `CommandPool` can allocate command buffers of a specific type only.
/// The allocated command buffers are associated with the creating command queue.
pub trait CommandPool {
    type Queue: CommandQueue;
    type PoolBuffer: command::CommandBuffer;

    /// Get a command buffer for recording.
    ///
    /// You can only record to one command buffer per pool at the same time.
    /// If more command buffers are requested than allocated, new buffers will be reserved.
    /// The command buffer will be returned in 'recording' state.
    fn acquire_command_buffer<'a>(&'a mut self) -> command::Encoder<'a, Self::PoolBuffer>;

    /// Reset the command pool and the corresponding command buffers.
    // TODO: synchronization: can't free pool if command buffer still in use (pool memory still in use)
    fn reset(&mut self);

    /// Reserve an additional amount of command buffers.
    fn reserve(&mut self, additional: usize);
}

/// A `Surface` abstracts the surface of a native window, which will be presented
pub trait Surface {
    type Queue;
    type SwapChain: SwapChain;

    fn build_swapchain<T: format::RenderFormat>(&self, present_queue: &Self::Queue)
        -> Self::SwapChain;
}

/// Handle to a backbuffer of the swapchain.
pub struct Frame(usize);

impl Frame {
    #[doc(hidden)]
    pub unsafe fn new(id: usize) -> Self {
        Frame(id)
    }

    pub fn id(&self) -> usize { self.0 }
}

/// Synchronization primitives which will be signaled once a frame got retrieved.
///
/// The semaphore or fence _must_ be unsignaled.
pub enum FrameSync<'a, R: Resources> {
    Semaphore(&'a R::Semaphore),
    Fence(&'a R::Fence)
}

/// The `SwapChain` is the backend representation of the surface.
/// It consists of multiple buffers, which will be presented on the surface.
pub trait SwapChain {
    type Image;
    type R: Resources;

    fn get_images(&mut self) -> &[Self::Image];
    fn acquire_frame(&mut self, sync: FrameSync<Self::R>) -> Frame;
    fn present(&mut self);
}

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

/// Different types of a specific API.
pub trait Backend {
    type CommandQueue: CommandQueue;
    type Factory: Factory<Self::Resources>;
    type Instance: Instance;
    type Adapter: Adapter;
    type Resources: Resources;
    type Surface: Surface;
    type SwapChain: SwapChain;
}
