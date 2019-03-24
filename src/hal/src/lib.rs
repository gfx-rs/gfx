#![deny(missing_docs, unused)]

//! Low-level graphics abstraction for Rust. Mostly operates on data, not types.
//! Designed for use by libraries and higher-level abstractions only.

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate failure;

#[cfg(feature = "mint")]
extern crate mint;

#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

use std::any::Any;
use std::error::Error;
use std::fmt;
use std::hash::Hash;

//TODO: reconsider what is publicly exported

pub use self::adapter::{
    Adapter, AdapterInfo, MemoryProperties, MemoryType, MemoryTypeId, PhysicalDevice, QueuePriority,
};
pub use self::device::Device;
pub use self::pool::CommandPool;
pub use self::pso::DescriptorPool;
pub use self::queue::{
    Capability, CommandQueue, Compute, General, Graphics, QueueFamily, QueueGroup, QueueType,
    Submission, Supports, Transfer,
};
pub use self::window::{
    AcquireError, Backbuffer, CompositeAlpha, PresentMode, Surface, SurfaceCapabilities,
    SwapImageIndex, Swapchain, SwapchainConfig,
};

pub mod adapter;
pub mod buffer;
pub mod command;
pub mod device;
pub mod error;
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
/// Draw number of instances.
pub type InstanceCount = u32;
/// Indirect draw calls count.
pub type DrawCount = u32;
/// Number of vertices in a patch
pub type PatchSize = u8;
/// Number of work groups.
pub type WorkGroupCount = [u32; 3];

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

bitflags! {
    /// Features that the device supports.
    /// These only include features of the core interface and not API extensions.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Features: u64 {
        /// Bit mask of Vulkan Core features.
        const CORE_MASK   = 0x0FFF_FFFF_FFFF_FFFF;
        /// Bit mask of Vulkan Portability features.
        const PORTABILITY_MASK  = 0xF000_0000_0000_0000;

        /// Support for robust buffer access.
        /// Buffer access by SPIR-V shaders is checked against the buffer/image boundaries.
        const ROBUST_BUFFER_ACCESS = 0x000_0000_0000_0001;
        /// Support the full 32-bit range of indexed for draw calls.
        /// If not supported, the maximum index value is determined by `Limits::max_draw_index_value`.
        const FULL_DRAW_INDEX_U32 = 0x000_0000_0000_0002;
        /// Support cube array image views.
        const IMAGE_CUBE_ARRAY = 0x000_0000_0000_0004;
        /// Support different color blending settings per attachments on graphics pipeline creation.
        const INDEPENDENT_BLENDING = 0x000_0000_0000_0008;
        /// Support geometry shader.
        const GEOMETRY_SHADER = 0x000_0000_0000_0010;
        /// Support tessellation shaders.
        const TESSELLATION_SHADER = 0x000_0000_0000_0020;
        /// Support per-sample shading and multisample interpolation.
        const SAMPLE_RATE_SHADING = 0x000_0000_0000_0040;
        /// Support dual source blending.
        const DUAL_SRC_BLENDING = 0x000_0000_0000_0080;
        /// Support logic operations.
        const LOGIC_OP = 0x000_0000_0000_0100;
        /// Support multiple draws per indirect call.
        const MULTI_DRAW_INDIRECT = 0x000_0000_0000_0200;
        /// Support indirect drawing with first instance value.
        /// If not supported the first instance value **must** be 0.
        const DRAW_INDIRECT_FIRST_INSTANCE = 0x00_0000_0000_0400;
        /// Support depth clamping.
        const DEPTH_CLAMP = 0x000_0000_0000_0800;
        /// Support depth bias clamping.
        const DEPTH_BIAS_CLAMP = 0x000_0000_0000_1000;
        /// Support non-fill polygon modes.
        const NON_FILL_POLYGON_MODE = 0x000_0000_0000_2000;
        /// Support depth bounds test.
        const DEPTH_BOUNDS = 0x000_0000_0000_4000;
        /// Support lines with width other than 1.0.
        const LINE_WIDTH = 0x000_0000_0000_8000;
        /// Support points with size greater than 1.0.
        const POINT_SIZE = 0x000_0000_0001_0000;
        /// Support replacing alpha values with 1.0.
        const ALPHA_TO_ONE = 0x000_0000_0002_0000;
        /// Support multiple viewports and scissors.
        const MULTI_VIEWPORTS = 0x000_0000_0004_0000;
        /// Support anisotropic filtering.
        const SAMPLER_ANISOTROPY = 0x000_0000_0008_0000;
        /// Support ETC2 texture compression formats.
        const FORMAT_ETC2 = 0x000_0000_0010_0000;
        /// Support ASTC (LDR) texture compression formats.
        const FORMAT_ASTC_LDR = 0x000_0000_0020_0000;
        /// Support BC texture compression formats.
        const FORMAT_BC = 0x000_0000_0040_0000;
        /// Support precise occlusion queries, returning the actual number of samples.
        /// If not supported, queries return a non-zero value when at least **one** sample passes.
        const PRECISE_OCCLUSION_QUERY = 0x000_0000_0080_0000;
        /// Support query of pipeline statistics.
        const PIPELINE_STATISTICS_QUERY = 0x000_0000_0100_0000;
        /// Support unordered access stores and atomic ops in the vertex, geometry
        /// and tessellation shader stage.
        /// If not supported, the shader resources **must** be annotated as read-only.
        const VERTEX_STORES_AND_ATOMICS = 0x000_0000_0200_0000;
        /// Support unordered access stores and atomic ops in the fragment shader stage
        /// If not supported, the shader resources **must** be annotated as read-only.
        const FRAGMENT_STORES_AND_ATOMICS = 0x000_0000_0400_0000;
        ///
        const SHADER_TESSELLATION_AND_GEOMETRY_POINT_SIZE = 0x000_0000_0800_0000;
        ///
        const SHADER_IMAGE_GATHER_EXTENDED = 0x000_0000_1000_0000;
        ///
        const SHADER_STORAGE_IMAGE_EXTENDED_FORMATS = 0x000_0000_2000_0000;
        ///
        const SHADER_STORAGE_IMAGE_MULTISAMPLE = 0x000_0000_4000_0000;
        ///
        const SHADER_STORAGE_IMAGE_READ_WITHOUT_FORMAT = 0x000_0000_8000_0000;
        ///
        const SHADER_STORAGE_IMAGE_WRITE_WITHOUT_FORMAT = 0x000_0001_0000_0000;
        ///
        const SHADER_UNIFORM_BUFFER_ARRAY_DYNAMIC_INDEXING = 0x000_0002_0000_0000;
        ///
        const SHADER_SAMPLED_IMAGE_ARRAY_DYNAMIC_INDEXING = 0x000_0004_0000_0000;
        ///
        const SHADER_STORAGE_BUFFER_ARRAY_DYNAMIC_INDEXING = 0x000_0008_0000_0000;
        ///
        const SHADER_STORAGE_IMAGE_ARRAY_DYNAMIC_INDEXING = 0x000_0010_0000_0000;
        ///
        const SHADER_CLIP_DISTANCE = 0x000_0020_0000_0000;
        ///
        const SHADER_CULL_DISTANCE = 0x000_0040_0000_0000;
        ///
        const SHADER_FLOAT64 = 0x000_0080_0000_0000;
        ///
        const SHADER_INT64 = 0x000_0100_0000_0000;
        ///
        const SHADER_INT16 = 0x000_0200_0000_0000;
        ///
        const SHADER_RESOURCE_RESIDENCY = 0x000_0400_0000_0000;
        ///
        const SHADER_RESOURCE_MIN_LOD = 0x000_0800_0000_0000;
        ///
        const SPARSE_BINDING = 0x000_1000_0000_0000;
        ///
        const SPARSE_RESIDENCY_BUFFER = 0x000_2000_0000_0000;
        ///
        const SPARSE_RESIDENCY_IMAGE_2D = 0x000_4000_0000_0000;
        ///
        const SPARSE_RESIDENCY_IMAGE_3D = 0x000_8000_0000_0000;
        ///
        const SPARSE_RESIDENCY_2_SAMPLES = 0x001_0000_0000_0000;
        ///
        const SPARSE_RESIDENCY_4_SAMPLES = 0x002_0000_0000_0000;
        ///
        const SPARSE_RESIDENCY_8_SAMPLES = 0x004_0000_0000_0000;
        ///
        const SPARSE_RESIDENCY_16_SAMPLES = 0x008_0000_0000_0000;
        ///
        const SPARSE_RESIDENCY_ALIASED = 0x010_0000_0000_0000;
        ///
        const VARIABLE_MULTISAMPLE_RATE = 0x020_0000_0000_0000;
        ///
        const INHERITED_QUERIES = 0x040_0000_0000_0000;

        /// Support triangle fan primitive topology.
        const TRIANGLE_FAN = 0x1000_0000_0000_0000;
        /// Support separate stencil reference values for front and back sides.
        const SEPARATE_STENCIL_REF_VALUES = 0x2000_0000_0000_0000;
        /// Support manually specified vertex attribute rates (divisors).
        const INSTANCE_RATE = 0x4000_0000_0000_0000;
        /// Support non-zero mipmap bias on samplers.
        const SAMPLER_MIP_LOD_BIAS = 0x8000_0000_0000_0000;
    }
}

/// Resource limits of a particular graphics device.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Limits {
    /// Maximum supported image 1D size.
    pub max_image_1d_size: image::Size,
    /// Maximum supported image 2D size.
    pub max_image_2d_size: image::Size,
    /// Maximum supported image 3D size.
    pub max_image_3d_size: image::Size,
    /// Maximum supported image cube size.
    pub max_image_cube_size: image::Size,
    /// Maximum supporter image array size.
    pub max_image_array_layers: image::Layer,
    /// Maximum number of elements for the BufferView to see.
    pub max_texel_elements: usize,
    ///
    pub max_uniform_buffer_range: buffer::Offset,
    ///
    pub max_storage_buffer_range: buffer::Offset,
    ///
    pub max_push_constants_size: usize,
    ///
    pub max_memory_allocation_count: usize,
    ///
    pub max_sampler_allocation_count: usize,
    ///
    pub max_bound_descriptor_sets: pso::DescriptorSetIndex,

    ///
    pub max_per_stage_descriptor_samplers: usize,
    ///
    pub max_per_stage_descriptor_uniform_buffers: usize,
    ///
    pub max_per_stage_descriptor_storage_buffers: usize,
    ///
    pub max_per_stage_descriptor_sampled_images: usize,
    ///
    pub max_per_stage_descriptor_storage_images: usize,
    ///
    pub max_per_stage_descriptor_input_attachments: usize,
    ///
    pub max_per_stage_resources: usize,

    ///
    pub max_descriptor_set_samplers: usize,
    ///
    pub max_descriptor_set_uniform_buffers: usize,
    ///
    pub max_descriptor_set_uniform_buffers_dynamic: usize,
    ///
    pub max_descriptor_set_storage_buffers: usize,
    ///
    pub max_descriptor_set_storage_buffers_dynamic: usize,
    ///
    pub max_descriptor_set_sampled_images: usize,
    ///
    pub max_descriptor_set_storage_images: usize,
    ///
    pub max_descriptor_set_input_attachments: usize,

    /// Maximum number of vertex input attributes that can be specified for a graphics pipeline.
    pub max_vertex_input_attributes: usize,
    /// Maximum number of vertex buffers that can be specified for providing vertex attributes to a graphics pipeline.
    pub max_vertex_input_bindings: usize,
    /// Maximum vertex input attribute offset that can be added to the vertex input binding stride.
    pub max_vertex_input_attribute_offset: usize,
    /// Maximum vertex input binding stride that can be specified in a vertex input binding.
    pub max_vertex_input_binding_stride: usize,
    /// Maximum number of components of output variables which can be output by a vertex shader.
    pub max_vertex_output_components: usize,

    /// Maximum number of vertices for each patch.
    pub max_patch_size: PatchSize,
    ///
    pub max_geometry_shader_invocations: usize,
    ///
    pub max_geometry_input_components: usize,
    ///
    pub max_geometry_output_components: usize,
    ///
    pub max_geometry_output_vertices: usize,
    ///
    pub max_geometry_total_output_components: usize,
    ///
    pub max_fragment_input_components: usize,
    ///
    pub max_fragment_output_attachments: usize,
    ///
    pub max_fragment_dual_source_attachments: usize,
    ///
    pub max_fragment_combined_output_resources: usize,

    ///
    pub max_compute_shared_memory_size: usize,
    ///
    pub max_compute_work_group_count: WorkGroupCount,
    ///
    pub max_compute_work_group_invocations: usize,
    ///
    pub max_compute_work_group_size: [u32; 3],

    ///
    pub max_draw_indexed_index_value: IndexCount,
    ///
    pub max_draw_indirect_count: InstanceCount,

    ///
    pub max_sampler_lod_bias: f32,
    /// Maximum degree of sampler anisotropy.
    pub max_sampler_anisotropy: f32,

    /// Maximum number of viewports.
    pub max_viewports: usize,
    ///
    pub max_viewport_dimensions: [image::Size; 2],
    ///
    pub max_framebuffer_extent: image::Extent,

    ///
    pub min_memory_map_alignment: usize,
    ///
    pub buffer_image_granularity: buffer::Offset,
    /// The alignment of the start of buffer used as a texel buffer, in bytes, non-zero.
    pub min_texel_buffer_offset_alignment: buffer::Offset,
    /// The alignment of the start of buffer used for uniform buffer updates, in bytes, non-zero.
    pub min_uniform_buffer_offset_alignment: buffer::Offset,
    /// The alignment of the start of buffer used as a storage buffer, in bytes, non-zero.
    pub min_storage_buffer_offset_alignment: buffer::Offset,
    /// Number of samples supported for color attachments of framebuffers (floating/fixed point).
    pub framebuffer_color_samples_count: image::NumSamples,
    /// Number of samples supported for depth attachments of framebuffers.
    pub framebuffer_depth_samples_count: image::NumSamples,
    /// Number of samples supported for stencil attachments of framebuffers.
    pub framebuffer_stencil_samples_count: image::NumSamples,
    /// Maximum number of color attachments that can be used by a subpass in a render pass.
    pub max_color_attachments: usize,
    ///
    pub standard_sample_locations: bool,
    /// The alignment of the start of the buffer used as a GPU copy source, in bytes, non-zero.
    pub optimal_buffer_copy_offset_alignment: buffer::Offset,
    /// The alignment of the row pitch of the texture data stored in a buffer that is
    /// used in a GPU copy operation, in bytes, non-zero.
    pub optimal_buffer_copy_pitch_alignment: buffer::Offset,
    /// Size and alignment in bytes that bounds concurrent access to host-mapped device memory.
    pub non_coherent_atom_size: usize,

    /// The alignment of the vertex buffer stride.
    pub min_vertex_input_binding_stride_alignment: buffer::Offset,
}

/// Describes the type of geometric primitives,
/// created from vertex data.
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

/// An enum describing the type of an index value in a slice's index buffer
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum IndexType {
    U16,
    U32,
}

/// An instantiated backend.
///
/// Any startup the backend needs to perform will be done when creating the type that implements
/// `Instance`.
///
/// # Examples
///
/// ```rust
/// # extern crate gfx_backend_empty;
/// # extern crate gfx_hal;
/// use gfx_backend_empty as backend;
/// use gfx_hal as hal;
///
/// // Create a concrete instance of our backend (this is backend-dependent and may be more
/// // complicated for some backends).
/// let instance = backend::Instance;
/// // We can get a list of the available adapters, which are either physical graphics
/// // devices, or virtual adapters. Because we are using the dummy `empty` backend,
/// // there will be nothing in this list.
/// for (idx, adapter) in hal::Instance::enumerate_adapters(&instance).iter().enumerate() {
///     println!("Adapter {}: {:?}", idx, adapter.info);
/// }
/// ```
pub trait Instance: Any + Send + Sync {
    /// Associated backend type of this instance.
    type Backend: Backend;
    /// Return all available adapters.
    fn enumerate_adapters(&self) -> Vec<Adapter<Self::Backend>>;
}

/// The `Backend` trait wraps together all the types needed
/// for a graphics backend. Each backend module, such as OpenGL
/// or Metal, will implement this trait with its own concrete types.
#[allow(missing_docs)]
pub trait Backend: 'static + Sized + Eq + Clone + Hash + fmt::Debug + Any + Send + Sync {
    //type Instance:          Instance<Self>;
    type PhysicalDevice: PhysicalDevice<Self>;
    type Device: Device<Self>;

    type Surface: Surface<Self>;
    type Swapchain: Swapchain<Self>;

    type QueueFamily: QueueFamily;
    type CommandQueue: queue::RawCommandQueue<Self>;
    type CommandBuffer: command::RawCommandBuffer<Self>;

    type ShaderModule: fmt::Debug + Any + Send + Sync;
    type RenderPass: fmt::Debug + Any + Send + Sync;
    type Framebuffer: fmt::Debug + Any + Send + Sync;

    type Memory: fmt::Debug + Any + Send + Sync;
    type CommandPool: pool::RawCommandPool<Self>;

    type Buffer: fmt::Debug + Any + Send + Sync;
    type BufferView: fmt::Debug + Any + Send + Sync;
    type Image: fmt::Debug + Any + Send + Sync;
    type ImageView: fmt::Debug + Any + Send + Sync;
    type Sampler: fmt::Debug + Any + Send + Sync;

    type ComputePipeline: fmt::Debug + Any + Send + Sync;
    type GraphicsPipeline: fmt::Debug + Any + Send + Sync;
    type PipelineCache: fmt::Debug + Any + Send + Sync;
    type PipelineLayout: fmt::Debug + Any + Send + Sync;
    type DescriptorPool: pso::DescriptorPool<Self>;
    type DescriptorSet: fmt::Debug + Any + Send + Sync;
    type DescriptorSetLayout: fmt::Debug + Any + Send + Sync;

    type Fence: fmt::Debug + Any + Send + Sync;
    type Semaphore: fmt::Debug + Any + Send + Sync;
    type QueryPool: fmt::Debug + Any + Send + Sync;
}

/// Marks that an error occurred submitting a command to a command buffer.
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

/// Submission result for DX11 backend.  Currently mostly unused.
pub type SubmissionResult<T> = Result<T, SubmissionError>;

/// Represents a combination of a logical device and the
/// hardware queues it provides.
///
/// This structure is typically created using an `Adapter`.
pub struct Gpu<B: Backend> {
    /// Logical device for a given backend.
    pub device: B::Device,
    /// The command queues that the device provides.
    pub queues: queue::Queues<B>,
}
