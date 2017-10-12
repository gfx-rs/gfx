#![deny(missing_docs)]

//! Low-level graphics abstraction for Rust. Mostly operates on data, not types.
//! Designed for use by libraries and higher-level abstractions only.

#[macro_use]
extern crate bitflags;
extern crate draw_state;
//#[macro_use]
//extern crate log;
extern crate smallvec;

#[cfg(feature = "mint")]
extern crate mint;

#[cfg(feature = "serialize")]
#[macro_use]
extern crate serde;

use std::any::Any;
use std::error::Error;
use std::fmt::{self, Debug};
use std::hash::Hash;

pub use self::adapter::{Adapter, AdapterInfo};
pub use self::command::{RawCommandBuffer};
pub use self::device::Device;
pub use self::pool::{CommandPool, RawCommandPool, SubpassCommandPool};
pub use self::pso::{DescriptorPool};
pub use self::queue::{
    CommandQueue, QueueFamily, QueueType, RawCommandQueue, RawSubmission, Submission,
    General, Graphics, Compute, Transfer,
};
pub use self::window::{
    Backbuffer, Frame, FrameSync, Surface, SurfaceCapabilities, Swapchain, SwapchainConfig};
pub use draw_state::{state, target};

pub mod adapter;
pub mod buffer;
pub mod command;
pub mod device;
pub mod format;
pub mod image;
pub mod mapping;
pub mod memory;
pub mod pass;
pub mod pool;
pub mod pso;
pub mod queue;
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

impl Viewport {
    /// Construct a viewport from rectangle.
    pub fn from_rect(rect: target::Rect, near: f32, far: f32) -> Self {
        Viewport {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
            near,
            far,
        }
    }
}

/// Features that the device supports.
/// These only include features of the core interface and not API extensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Features {
    /// Support indirect drawing and dispatching.
    pub indirect_execution: bool,
    /// Support instanced drawing.
    pub draw_instanced: bool,
    /// Support offsets for instanced drawing with base instance.
    pub draw_instanced_base: bool,
    /// Support indexed drawing with base vertex.
    pub draw_indexed_base: bool,
    /// Support indexed, instanced drawing.
    pub draw_indexed_instanced: bool,
    /// Support indexed, instanced drawing with base vertex only.
    pub draw_indexed_instanced_base_vertex: bool,
    /// Support indexed, instanced drawing with base vertex and instance.
    pub draw_indexed_instanced_base: bool,
    /// Support manually specified vertex attribute rates (divisors).
    pub instance_rate: bool,
    /// Support base vertex offset for indexed drawing.
    pub vertex_base: bool,
    /// Support sRGB textures and rendertargets.
    pub srgb_color: bool,
    /// Support constant buffers.
    pub constant_buffer: bool,
    /// Support unordered-access views.
    pub unordered_access_view: bool,
    /// Support specifying the blend function and equation for each color target.
    pub separate_blending_slots: bool,
    /// Support accelerated buffer copy.
    pub copy_buffer: bool,
    /// Support separation of textures and samplers.
    pub sampler_objects: bool,
    /// Support sampler LOD bias.
    pub sampler_lod_bias: bool,
    /// Support anisotropic filtering.
    pub sampler_anisotropy: bool,
    /// Support setting border texel colors.
    pub sampler_border_color: bool,
}

/// Limits of the device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Limits {
    /// Maximum supported texture size.
    pub max_texture_size: usize,
    /// Maximum number of vertices for each patch.
    pub max_patch_size: PatchSize,
    /// Maximum number of viewports.
    pub max_viewports: usize,
    ///
    pub max_compute_group_count: [usize; 3],
    ///
    pub max_compute_group_size: [usize; 3],

    /// The alignment of the start of the buffer used as a GPU copy source, in bytes, non-zero.
    pub min_buffer_copy_offset_alignment: usize,
    /// The alignment of the row pitch of the texture data stored in a buffer that is
    /// used in a GPU copy operation, in bytes, non-zero.
    pub min_buffer_copy_pitch_alignment: usize,
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
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct MemoryType {
    /// Id of the memory type.
    pub id: usize,
    /// Properties of the associated memory.
    pub properties: memory::Properties,
    /// Index to the underlying memory heap in `Gpu::memory_heaps`
    pub heap_index: usize,
}

/// Basic backend instance trait.
pub trait Instance<B: Backend> {
    /// Enumerate all available adapters.
    fn enumerate_adapters(&self) -> Vec<B::Adapter>;
}

/// Different types of a specific API.
#[allow(missing_docs)]
pub trait Backend: 'static + Sized + Eq + Clone + Hash + Debug + Any {
    //type Instance:          Instance<Self>;
    type Adapter:             Adapter<Self>;
    type Device:              Device<Self>;

    type Surface:             Surface<Self>;
    type Swapchain:           Swapchain<Self>;

    type CommandQueue:        RawCommandQueue<Self>;
    type CommandBuffer:       RawCommandBuffer<Self>;
    type SubpassCommandBuffer;
    type QueueFamily:         QueueFamily;

    type ShaderModule:        Debug + Any + Send + Sync;
    type RenderPass:          Debug + Any + Send + Sync;
    type Framebuffer:         Debug + Any + Send + Sync;

    type Memory:              Debug + Any;
    type CommandPool:         RawCommandPool<Self>;
    type SubpassCommandPool:  SubpassCommandPool<Self>;

    type UnboundBuffer:       Debug + Any + Send + Sync;
    type Buffer:              Debug + Any + Send + Sync;
    type BufferView:          Debug + Any + Send + Sync;
    type UnboundImage:        Debug + Any + Send + Sync;
    type Image:               Debug + Any + Send + Sync;
    type ImageView:           Debug + Any + Send + Sync;
    type Sampler:             Debug + Any + Send + Sync;

    type ComputePipeline:     Debug + Any + Send + Sync;
    type GraphicsPipeline:    Debug + Any + Send + Sync;
    type PipelineLayout:      Debug + Any + Send + Sync;
    type DescriptorPool:      DescriptorPool<Self>;
    type DescriptorSet:       Debug + Any + Send + Sync;
    type DescriptorSetLayout: Debug + Any + Send + Sync;

    type Fence:               Debug + Any + Send + Sync;
    type Semaphore:           Debug + Any + Send + Sync;
}

#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum SubmissionError {}

impl fmt::Display for SubmissionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for SubmissionError {
    fn description(&self) -> &str {
        "Submission error"
    }
}

#[allow(missing_docs)]
pub type SubmissionResult<T> = Result<T, SubmissionError>;

/// Represents a handle to a physical device.
///
/// This structure is typically created using an `Adapter`.
pub struct Gpu<B: Backend> {
    /// Logical device.
    pub device: B::Device,
    /// General command queues.
    pub general_queues: Vec<CommandQueue<B, General>>,
    /// Graphics command queues.
    pub graphics_queues: Vec<CommandQueue<B, Graphics>>,
    /// Compute command queues.
    pub compute_queues: Vec<CommandQueue<B, Compute>>,
    /// Transfer command queues.
    pub transfer_queues: Vec<CommandQueue<B, Transfer>>,
    /// Types of memory.
    ///
    /// Each memory type is associated with one heap of `memory_heaps`.
    /// Multiple types can point to the same heap.
    pub memory_types: Vec<MemoryType>,
    /// Memory heaps with their size in bytes.
    pub memory_heaps: Vec<u64>,
}
