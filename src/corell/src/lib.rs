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
extern crate log;
extern crate draw_state;

use std::fmt::Debug;
use std::hash::Hash;
use std::any::Any;
use std::slice::Iter;
use std::ops::{Deref, DerefMut};

pub use draw_state::{state, target};
pub use self::factory::Factory;

pub mod command;
pub mod factory;
pub mod format;
pub mod memory;
pub mod pso;
pub mod shade;

/// Compile-time maximum number of color targets.
pub const MAX_COLOR_TARGETS: usize = 8; // Limited by D3D12

/// Draw vertex count.
pub type VertexCount = u32;
/// Draw number of instances
pub type InstanceCount = u32;
/// Number of vertices in a patch
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

pub struct Device<R: Resources, F: Factory<R>, Q: CommandQueue> {
    pub factory: F,
    pub general_queues: Vec<GeneralQueue<Q>>,
    pub graphics_queues: Vec<GraphicsQueue<Q>>,
    pub compute_queues: Vec<ComputeQueue<Q>>,
    pub transfer_queues: Vec<TransferQueue<Q>>,
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
        where I: Iterator<Item=(&'a Self::QueueFamily, u32)>;

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

pub trait CommandQueue {
    type CommandBuffers: CommandBuffers;

    unsafe fn submit(&mut self, cmd_buffer: &<<Self as CommandQueue>::CommandBuffers as CommandBuffers>::CommandBuffer);
}

pub struct GeneralQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> GeneralQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        GeneralQueue(queue)
    }

    pub fn submit_general(&mut self, cmd_buffer: &<<Q as CommandQueue>::CommandBuffers as CommandBuffers>::GeneralCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_graphics(&mut self, cmd_buffer: &<<Q as CommandQueue>::CommandBuffers as CommandBuffers>::GraphicsCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_compute(&mut self, cmd_buffer: &<<Q as CommandQueue>::CommandBuffers as CommandBuffers>::ComputeCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_tranfer(&mut self, cmd_buffer: &<<Q as CommandQueue>::CommandBuffers as CommandBuffers>::TransferCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
}

impl<Q: CommandQueue> Deref for GeneralQueue<Q> {
    type Target = Q;
    fn deref(&self) -> &Q {
        &self.0
    }
}
impl<Q: CommandQueue> DerefMut for GeneralQueue<Q> {
    fn deref_mut(&mut self) -> &mut Q {
        &mut self.0
    }
}

pub struct GraphicsQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> GraphicsQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        GraphicsQueue(queue)
    }

    pub fn submit_graphics(&mut self, cmd_buffer: &<<Q as CommandQueue>::CommandBuffers as CommandBuffers>::GraphicsCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_tranfer(&mut self, cmd_buffer: &<<Q as CommandQueue>::CommandBuffers as CommandBuffers>::TransferCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
}

impl<Q: CommandQueue> Deref for GraphicsQueue<Q> {
    type Target = Q;
    fn deref(&self) -> &Q {
        &self.0
    }
}
impl<Q: CommandQueue> DerefMut for GraphicsQueue<Q> {
    fn deref_mut(&mut self) -> &mut Q {
        &mut self.0
    }
}

pub struct ComputeQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> ComputeQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        ComputeQueue(queue)
    }

    pub fn submit_compute(&mut self, cmd_buffer: &<<Q as CommandQueue>::CommandBuffers as CommandBuffers>::ComputeCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
    pub fn submit_tranfer(&mut self, cmd_buffer: &<<Q as CommandQueue>::CommandBuffers as CommandBuffers>::TransferCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
}

impl<Q: CommandQueue> Deref for ComputeQueue<Q> {
    type Target = Q;
    fn deref(&self) -> &Q {
        &self.0
    }
}
impl<Q: CommandQueue> DerefMut for ComputeQueue<Q> {
    fn deref_mut(&mut self) -> &mut Q {
        &mut self.0
    }
}

pub struct TransferQueue<Q: CommandQueue>(Q);
impl<Q: CommandQueue> TransferQueue<Q> {
    #[doc(hidden)]
    pub unsafe fn new(queue: Q) -> Self {
        TransferQueue(queue)
    }

    pub fn submit_tranfer(&mut self, cmd_buffer: &<<Q as CommandQueue>::CommandBuffers as CommandBuffers>::TransferCommandBuffer) {
        unsafe { self.submit(&cmd_buffer) }
    }
}

impl<Q: CommandQueue> Deref for TransferQueue<Q> {
    type Target = Q;
    fn deref(&self) -> &Q {
        &self.0
    }
}
impl<Q: CommandQueue> DerefMut for TransferQueue<Q> {
    fn deref_mut(&mut self) -> &mut Q {
        &mut self.0
    }
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
}

/// The `SwapChain` is the backend representation of the surface.
/// It consists of multiple buffers, which will be presented on the surface.
pub trait SwapChain {
    fn acquire_frame(&mut self) -> Frame;
    fn present(&mut self);
}

/// Different resource types of a specific API. 
pub trait Resources:          Clone + Hash + Debug + Any {
    type Buffer:              Clone + Hash + Debug + Any + Send + Sync + Copy;
    type ShaderLib:           Clone + Hash + Debug + Any + Send + Sync;
    type RenderPass:          Clone + Hash + Debug + Any + Send + Sync;
    type PipelineSignature:   Clone + Hash + Debug + Any + Send + Sync;
    type PipelineStateObject: Clone + Hash + Debug + Any + Send + Sync;
    type Image:               Clone + Hash + Debug + Any + Send + Sync;
    type ShaderResourceView:  Clone + Hash + Debug + Any + Send + Sync + Copy;
    type UnorderedAccessView: Clone + Hash + Debug + Any + Send + Sync + Copy;
    type RenderTargetView:    Clone + Hash + Debug + Any + Send + Sync + Copy;
    type DepthStencilView:    Clone + Hash + Debug + Any + Send + Sync;
    type Sampler:             Clone + Hash + Debug + Any + Send + Sync + Copy;
}

pub trait CommandBuffers {
    type CommandBuffer;
    type GeneralCommandBuffer: Deref<Target=Self::CommandBuffer>;
    type GraphicsCommandBuffer: Deref<Target=Self::CommandBuffer>;
    type ComputeCommandBuffer: Deref<Target=Self::CommandBuffer>;
    type TransferCommandBuffer: Deref<Target=Self::CommandBuffer>;
}

/// Different types of a specific API.
pub trait Backend {
    type CommandBuffers: CommandBuffers;
    type CommandQueue: CommandQueue;
    type Factory: Factory<Self::Resources>;
    type Instance: Instance;
    type Adapter: Adapter;
    type Resources: Resources;
    type Surface: Surface;
    type SwapChain: SwapChain;
}
