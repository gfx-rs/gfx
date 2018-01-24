#![deny(missing_docs)]

//! Low-level graphics abstraction for Rust. Mostly operates on data, not types.
//! Designed for use by libraries and higher-level abstractions only.

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate failure;
extern crate smallvec;

#[cfg(feature = "mint")]
extern crate mint;

#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

use std::any::Any;
use std::error::Error;
use std::fmt::{self, Debug};
use std::hash::Hash;

pub use self::adapter::{
    Adapter, AdapterInfo, MemoryProperties, MemoryType, MemoryTypeId,
    PhysicalDevice, QueuePriority,
};
pub use self::device::Device;
pub use self::pool::CommandPool;
pub use self::pso::{DescriptorPool};
pub use self::queue::{
    CommandQueue, QueueGroup, QueueFamily, QueueType, Submission,
    Capability, General, Graphics, Compute, Transfer,
};
pub use self::window::{
    Backbuffer, Frame, FrameSync, Surface, SurfaceCapabilities, Swapchain, SwapchainConfig,
};

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
pub mod query;
pub mod queue;
pub mod range;
pub mod window;

#[doc(hidden)]
pub mod backend;

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

/// Features that the device supports.
/// These only include features of the core interface and not API extensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Features {
    // Core features

    /// Support for robust buffer access.
    /// Buffer access by SPIR-V shaders is checked against the buffer/image boundaries.
    pub robust_buffer_access: bool,
    /// Support the full 32-bit range of indexed for draw calls.
    /// If not supported, the maximum index value is determined by `Limits::max_draw_index_value`.
    pub full_draw_index_u32: bool,
    /// Support cube array image views.
    pub image_cube_array: bool,
    /// Support different color blending settings per attachments on graphics pipeline creation.
    pub independent_blending: bool,
    /// Support geometry shader.
    pub geometry_shader: bool,
    /// Support tessellation shaders.
    pub tessellation_shader: bool,
    /// Support per-sample shading and multisample interpolation.
    pub sample_rate_shading: bool,
    /// Support dual source blending.
    pub dual_src_blending: bool,
    /// Support logic operations.
    pub logic_op: bool,
    /// Support multiple draws per indirect call.
    pub multi_draw_indirect: bool,
    /// Support indirect drawing with first instance value.
    /// If not supported the first instance value **must** be 0.
    pub draw_indirect_first_instance: bool,
    /// Support depth clamping.
    pub depth_clamp: bool,
    /// Support depth bias clamping.
    pub depth_bias_clamp: bool,
    /// Support non-fill polygon modes.
    pub non_fill_polygon_mode: bool,
    /// Support depth bounds test.
    pub depth_bounds: bool,
    /// Support lines with width other than 1.0.
    pub line_width: bool,
    /// Support points with size greater than 1.0.
    pub point_size: bool,
    /// Support replacing alpha values with 1.0.
    pub alpha_to_one: bool,
    /// Support multiple viewports and scissors.
    pub multi_viewports: bool,
    /// Support anisotropic filtering.
    pub sampler_anisotropy: bool,
    /// Support ETC2 texture compression formats.
    pub format_etc2: bool,
    /// Support ASTC (LDR) texture compression formats.
    pub format_astc_ldr: bool,
    /// Support BC texture compression formats.
    pub format_bc: bool,
    /// Support precise occlusion queries, returning the actual number of samples.
    /// If not supported, queries return a non-zero value when at least **one** sample passes.
    pub precise_occlusion_query: bool,
    /// Support query of pipeline statistics.
    pub pipeline_statistics_query: bool,
    /// Support unordered access stores and atomic ops in the vertex, geometry
    /// and tessellation shader stage.
    /// If not supported, the shader resources **must** be annotated as read-only.
    pub vertex_stores_and_atomics: bool,
    /// Support unordered access stores and atomic ops in the fragment shader stage
    /// If not supported, the shader resources **must** be annotated as read-only.
    pub fragment_stores_and_atomics: bool,

    // Legacy features

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
    /// Support base vertex offset for indexed drawing.
    pub vertex_base: bool,
    /// Support sRGB textures and rendertargets.
    pub srgb_color: bool,
    /// Support constant buffers.
    pub constant_buffer: bool,
    /// Support unordered-access views.
    pub unordered_access_view: bool,
    /// Support accelerated buffer copy.
    pub copy_buffer: bool,
    /// Support separation of textures and samplers.
    pub sampler_objects: bool,
    /// Support sampler LOD bias.
    pub sampler_lod_bias: bool,
    /// Support setting border texel colors.
    pub sampler_border_color: bool,

    // Extension features

    /// Support manually specified vertex attribute rates (divisors).
    pub instance_rate: bool,
}

/// Limits of the device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    /// The alignment of the start of buffer used for uniform buffer updates, in bytes, non-zero.
    pub min_uniform_buffer_offset_alignment: usize,
}

/// Describes what geometric primitives are created from vertex data.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    /// Each sextuplet of vertices represent a single triangle with adjacency information. For
    /// example, with `[a, b, c, d, e, f]`, `a`, `c`, and `e` form a triangle, and `b`, `d`, and
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum IndexType {
    U16,
    U32,
}

/// Basic backend instance trait.
pub trait Instance {
    /// Associated backend type of this instance.
    type Backend: Backend;
    /// Enumerate all available adapters.
    fn enumerate_adapters(&self) -> Vec<Adapter<Self::Backend>>;
}

/// Different types of a specific API.
#[allow(missing_docs)]
pub trait Backend: 'static + Sized + Eq + Clone + Hash + Debug + Any {
    //type Instance:          Instance<Self>;
    type PhysicalDevice:      PhysicalDevice<Self>;
    type Device:              Device<Self>;

    type Surface:             Surface<Self>;
    type Swapchain:           Swapchain<Self>;

    type QueueFamily:         QueueFamily;
    type CommandQueue:        queue::RawCommandQueue<Self>;
    type CommandBuffer:       command::RawCommandBuffer<Self>;
    type SubpassCommandBuffer;

    type ShaderModule:        Debug + Any + Send + Sync;
    type RenderPass:          Debug + Any + Send + Sync;
    type Framebuffer:         Debug + Any + Send + Sync;

    type Memory:              Debug + Any;
    type CommandPool:         pool::RawCommandPool<Self>;
    type SubpassCommandPool:  pool::SubpassCommandPool<Self>;

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
    type QueryPool:           Debug + Any + Send + Sync;
}

#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    ///
    pub queues: queue::Queues<B>,
}
