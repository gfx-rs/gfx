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
extern crate smallvec;

#[cfg(feature = "mint")]
extern crate mint;

#[cfg(feature = "serialize")]
extern crate serde;
#[cfg(feature = "serialize")]
#[macro_use]
extern crate serde_derive;

use std::any::Any;
use std::error::Error;
use std::fmt::{self, Debug};
use std::hash::Hash;

pub use self::adapter::{Adapter, AdapterInfo};
pub use self::device::Device;
pub use self::pool::{ComputeCommandPool, GeneralCommandPool, GraphicsCommandPool, RawCommandPool,
                     SubpassCommandPool, TransferCommandPool};
pub use self::pso::{DescriptorPool};
pub use self::queue::{CommandQueue, QueueType, RawSubmission, Submission, QueueFamily,
                      ComputeQueue, GeneralQueue, GraphicsQueue, TransferQueue};
pub use self::window::{Backbuffer, Frame, FrameSync, Surface, SwapChain, SwapchainConfig,
                       WindowExt};
pub use draw_state::{state, target};

pub mod adapter;
pub mod buffer;
pub mod command;
// pub mod dummy;
pub mod device;
pub mod format;
pub mod handle;
pub mod mapping;
pub mod memory;
pub mod pass;
pub mod pool;
pub mod pso;
pub mod queue;
pub mod shade;
pub mod texture;
pub mod window;

/// Draw vertex count.
pub type VertexCount = u32;
/// Draw vertex base offset.
pub type VertexOffset = i32;
/// Draw number of indices.
pub type IndexCount = u32;
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

///
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub struct Viewport {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub near: f32,
    pub far: f32,
}


/// Features that the device supports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Capabilities {
    /// Maximum supported texture size.
    pub max_texture_size: usize,
    /// Maximum number of vertices for each patch.
    pub max_patch_size: PatchSize,

    /// Support instanced drawing.
    pub draw_instanced_supported: bool,
    /// Support offsets for instanced drawing with base instance.
    pub draw_instanced_base_supported: bool,
    /// Support indexed drawing with base vertex.
    pub draw_indexed_base_supported: bool,
    /// Support indexed, instanced drawing.
    pub draw_indexed_instanced_supported: bool,
    /// Support indexed, instanced drawing with base vertex only.
    pub draw_indexed_instanced_base_vertex_supported: bool,
    /// Support indexed, instanced drawing with base vertex and instance.
    pub draw_indexed_instanced_base_supported: bool,
    /// Support manually specified vertex attribute rates (divisors).
    pub instance_rate_supported: bool,
    /// Support base vertex offset for indexed drawing.
    pub vertex_base_supported: bool,
    /// Support sRGB textures and rendertargets.
    pub srgb_color_supported: bool,
    /// Support constant buffers.
    pub constant_buffer_supported: bool,
    /// Support unordered-access views.
    pub unordered_access_view_supported: bool,
    /// Support specifying the blend function and equation for each color target.
    pub separate_blending_slots_supported: bool,
    /// Support accelerated buffer copy.
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
pub trait Backend: 'static + Sized + Eq + Clone + Hash + Debug + Any {
    type Adapter: Adapter<Self>;
    type CommandQueue: CommandQueue<Self>;
    type Device: Device<Self>;
    type QueueFamily: QueueFamily;
    type SubmitInfo: Clone + Send;
    type DescriptorPool: DescriptorPool<Self>;

    type RawCommandBuffer: command::RawCommandBuffer<Self>;
    type SubpassCommandBuffer;

    type RawCommandPool: RawCommandPool<Self>;
    type SubpassCommandPool: SubpassCommandPool<Self>;

    type Buffer:              Debug + Any + Send + Sync + Eq + Hash;
    type ShaderLib:           Debug + Any + Send + Sync;
    type ShaderResourceView:  Debug + Any + Send + Sync + Clone + Hash + Eq;
    type UnorderedAccessView: Debug + Any + Send + Sync + Clone + Hash + Eq;
    type RenderTargetView:    Debug + Any + Send + Sync + Clone;
    type DepthStencilView:    Debug + Any + Send + Sync + Clone;
    type Sampler:             Debug + Any + Send + Sync;
    type Image:               Debug + Any + Send + Sync + Eq + Hash;
    type ComputePipeline:     Debug + Any + Send + Sync;
    type GraphicsPipeline:    Debug + Any + Send + Sync;
    type PipelineLayout:      Debug + Any + Send + Sync;
    type DescriptorHeap:      Debug + Any;
    type DescriptorSet:       Debug + Any + Send + Sync;
    type DescriptorSetLayout: Debug + Any;
    type Fence:               Debug + Any + Send + Sync;
    type Semaphore:           Debug + Any + Send + Sync;
    type Mapping:             Debug + Any + Send + Sync + mapping::Gate<Self>;
    type RenderPass:          Debug + Any + Send + Sync;
    type FrameBuffer:         Debug + Any + Send + Sync;
}

/*
    type UnboundBuffer:       Debug + Any + Send + Sync;
    type UnboundImage:        Debug + Any + Send + Sync;
    type ConstantBufferView:  Debug + Any + Send + Sync;
    type Heap:                Debug + Any;
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
            AccessOverlap => "A resource access overlaps with another",
        }
    }
}

#[allow(missing_docs)]
pub type SubmissionResult<T> = Result<T, SubmissionError>;

///
pub struct Gpu<B: Backend> {
    /// Logical device.
    pub device: B::Device,
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
}

/// Main entry point for window-less backend initialization.
pub trait Headless<B: Backend> {
    /// Associated `Adapter` type.
    type Adapter: Adapter<B>;

    /// Enumerate all available adapters.
    fn get_adapters(&mut self) -> Vec<Self::Adapter>;
}
