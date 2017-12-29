//! # Device
//!
//! This module exposes the `Device` trait, used for creating and managing graphics resources, and
//! includes several items to facilitate this.

use std::{fmt, mem, slice};
use std::error::Error;
use std::ops::Range;
use {buffer, format, image, mapping, pass, pso, query};
use pool::{CommandPool, CommandPoolCreateFlags};
use queue::QueueGroup;
use {Backend, MemoryTypeId};
use memory::Requirements;
use window::{Backbuffer, SwapchainConfig};


/// Error allocating memory.
#[derive(Clone, PartialEq, Debug)]
pub struct OutOfMemory;

impl fmt::Display for OutOfMemory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Out of memory")
    }
}

impl Error for OutOfMemory {
    fn description(&self) -> &str {
        "Out of memory"
    }
}

/// Error binding a resource to memory allocation.
#[derive(Clone, PartialEq, Debug)]
pub enum BindError {
    ///
    WrongMemory,
    ///
    OutOfBounds,
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl Error for BindError {
    fn description(&self) -> &str {
        match *self {
            BindError::WrongMemory => "Unsupported memory allocation for the requirements",
            BindError::OutOfBounds => "Not enough space in the memory allocation",
        }
    }
}

/// Specifies the waiting targets.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WaitFor {
    /// Wait for any target.
    Any,
    /// Wait for all targets at once.
    All,
}

///
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Extent {
    ///
    pub width: u32,
    ///
    pub height: u32,
    ///
    pub depth: u32,
}

/// An error from creating a shader module.
#[derive(Clone, Debug, PartialEq)]
pub enum ShaderError {
    /// The shader failed to compile.
    CompilationFailed(String),
    /// Missing entry point.
    MissingEntryPoint(String),
    /// Mismatch of interface (e.g missing push constants).
    InterfaceMismatch(String),
}

/// An error from creating a framebuffer.
#[derive(Clone, Debug, PartialEq)]
pub struct FramebufferError;

/// # Overview
///
/// A `Device` is responsible for creating and managing resources for the physical device
/// it was created from.
///
/// ## Resource Construction and Handling
///
/// This device structure can then be used to create and manage different resources, like buffers,
/// shader programs and textures. See the individual methods for more information.
///
/// ## Mutability
///
/// All the methods get `&self`. Any internal mutability of the `Device` is hidden from the user.
///
/// ## Synchronization
///
/// `Device` should be usable concurrently from multiple threads. The `Send` and `Sync` bounds
/// are not enforced at the HAL level due to OpenGL constraint (to be revised). Users can still
/// benefit from the backends that support synchronization of the `Device`.
///
pub trait Device<B: Backend> {
    /// Allocates a memory segment of a specified type.
    ///
    /// There is only a limited amount of allocations allowed depending on the implementation!
    ///
    /// # Arguments
    ///
    /// * `memory_type` - Index of the memory type in the memory properties of the associated physical device.
    /// * `size` - Size of the allocation.
    fn allocate_memory(&self, memory_type: MemoryTypeId, size: u64) -> Result<B::Memory, OutOfMemory>;

    ///
    fn free_memory(&self, B::Memory);

    /// Creates a new command pool for a given queue family.
    ///
    /// *Note*: the family has to be associated by one as the `Gpu::queue_groups`.
    fn create_command_pool(&self, &B::QueueFamily, CommandPoolCreateFlags) -> B::CommandPool;

    /// Creates a strongly typed command pool wrapper.
    fn create_command_pool_typed<C>(
        &self,
        group: &QueueGroup<B, C>,
        flags: CommandPoolCreateFlags,
        max_buffers: usize,
    ) -> CommandPool<B, C> {
        let raw = self.create_command_pool(&group.family, flags);
        CommandPool::new(raw, max_buffers)
    }

    /// Destroys a command pool.
    fn destroy_command_pool(&self, B::CommandPool);

    ///
    fn create_render_pass(
        &self,
        &[pass::Attachment],
        &[pass::SubpassDesc],
        &[pass::SubpassDependency],
    ) -> B::RenderPass;

    ///
    fn destroy_renderpass(&self, B::RenderPass);

    /// Create a new pipeline layout.
    ///
    /// # Arguments
    ///
    /// * `set_layouts` - Descriptor set layouts
    /// * `push_constants` - Ranges of push constants. A shader stage may only contain one push
    ///     constant block. The length of the range indicates the number of u32 constants occupied
    ///     by the push constant block.
    fn create_pipeline_layout(
        &self,
        set_layouts: &[&B::DescriptorSetLayout],
        push_constants: &[(pso::ShaderStageFlags, Range<u32>)]
    ) -> B::PipelineLayout;

    ///
    fn destroy_pipeline_layout(&self, B::PipelineLayout);

    /// Create graphics pipelines.
    fn create_graphics_pipelines<'a>(
        &self,
        &[pso::GraphicsPipelineDesc<'a, B>],
    ) -> Vec<Result<B::GraphicsPipeline, pso::CreationError>>;

    /// Destroys a graphics pipeline.
    ///
    /// The graphics pipeline shouldn't be destroyed before any submitted command buffer,
    /// which references the graphics pipeline, has finished execution.
    fn destroy_graphics_pipeline(&self, B::GraphicsPipeline);

    /// Create compute pipelines.
    fn create_compute_pipelines<'a>(
        &self,
        &[pso::ComputePipelineDesc<'a, B>],
    ) -> Vec<Result<B::ComputePipeline, pso::CreationError>>;

    /// Destroys a compute pipeline.
    ///
    /// The compute pipeline shouldn't be destroyed before any submitted command buffer,
    /// which references the compute pipeline, has finished execution.
    fn destroy_compute_pipeline(&self, B::ComputePipeline);

    ///
    fn create_framebuffer(
        &self,
        &B::RenderPass,
        &[&B::ImageView],
        Extent,
    ) -> Result<B::Framebuffer, FramebufferError>;

    /// Destroys a framebuffer.
    ///
    /// The framebuffer shouldn't be destroy before any submitted command buffer,
    /// which references the framebuffer, has finished execution.
    fn destroy_framebuffer(&self, B::Framebuffer);

    ///
    fn create_shader_module(
        &self, spirv_data: &[u8]
    ) -> Result<B::ShaderModule, ShaderError>;

    ///
    fn destroy_shader_module(&self, B::ShaderModule);

    /// Create a new buffer (unbound).
    ///
    /// The created buffer won't have associated memory until `bind_buffer_memory` is called.
    fn create_buffer(
        &self, size: u64, buffer::Usage,
    ) -> Result<B::UnboundBuffer, buffer::CreationError>;

    ///
    fn get_buffer_requirements(&self, &B::UnboundBuffer) -> Requirements;

    /// Bind memory to a buffer.
    ///
    /// The unbound buffer will be consumed because the binding is *immutable*.
    /// Be sure to check that there is enough memory available for the buffer.
    /// Use `get_buffer_requirements` to acquire the memory requirements.
    fn bind_buffer_memory(
        &self, &B::Memory, offset: u64, B::UnboundBuffer
    ) -> Result<B::Buffer, BindError>;

    /// Destroys a buffer.
    ///
    /// The buffer shouldn't be destroyed before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_buffer(&self, B::Buffer);

    ///
    fn create_buffer_view(
        &self, &B::Buffer, Option<format::Format>, Range<u64>
    ) -> Result<B::BufferView, buffer::ViewError>;

    ///
    fn destroy_buffer_view(&self, B::BufferView);

    ///
    fn create_image(
        &self, image::Kind, image::Level, format::Format, image::Usage,
    ) -> Result<B::UnboundImage, image::CreationError>;

    ///
    fn get_image_requirements(&self, &B::UnboundImage) -> Requirements;

    ///
    fn bind_image_memory(
        &self, &B::Memory, offset: u64, B::UnboundImage
    ) -> Result<B::Image, BindError>;

    /// Destroys an image.
    ///
    /// The image shouldn't be destroyed before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_image(&self, B::Image);

    ///
    fn create_image_view(
        &self,
        &B::Image,
        format::Format,
        format::Swizzle,
        image::SubresourceRange,
    ) -> Result<B::ImageView, image::ViewError>;

    ///
    fn destroy_image_view(&self, B::ImageView);

    ///
    fn create_sampler(&self, image::SamplerInfo) -> B::Sampler;

    ///
    fn destroy_sampler(&self, B::Sampler);

    /// Create a descriptor pool.
    ///
    /// Descriptor pools allow allocation of descriptor sets.
    /// The pool can't be modified directly, only trough updating descriptor sets.
    fn create_descriptor_pool(
        &self,
        max_sets: usize,
        &[pso::DescriptorRangeDesc],
    ) -> B::DescriptorPool;

    ///
    fn destroy_descriptor_pool(&self, B::DescriptorPool);

    /// Create a descriptor set layout.
    fn create_descriptor_set_layout(
        &self,
        &[pso::DescriptorSetLayoutBinding],
    ) -> B::DescriptorSetLayout;

    ///
    fn destroy_descriptor_set_layout(&self, B::DescriptorSetLayout);

    ///
    // TODO: copies
    fn update_descriptor_sets(&self, &[pso::DescriptorSetWrite<B>]);

    // TODO: mapping requires further looking into.
    // vulkan requires non-coherent mapping to round the range delimiters
    // Nested mapping is not allowed in vulkan.
    // How to handle it properly for backends? Explicit synchronization?

    /// Acquire access to the buffer mapping.
    ///
    /// If you will read, you have to specify in which range.
    ///
    /// While holding this access, you hold CPU-side exclusive access.
    /// You must ensure that there is no GPU access to the buffer in the meantime.
    fn acquire_mapping_raw(&self, buf: &B::Buffer, read: Option<Range<u64>>)
        -> Result<*mut u8, mapping::Error>;

    /// Release access to the buffer mapping.
    ///
    /// If you wrote, you have to specify in which range.
    fn release_mapping_raw(&self, buf: &B::Buffer, wrote: Option<Range<u64>>);

    /// Acquire a mapping Reader
    ///
    /// The accessible slice will correspond to the specified range (in bytes).
    /// See `acquire_mapping_raw` for more information.
    fn acquire_mapping_reader<'a, T>(&self, buffer: &'a B::Buffer, range: Range<u64>)
        -> Result<mapping::Reader<'a, B, T>, mapping::Error>
    where
        T: Copy,
    {
        let count = (range.end - range.start) as usize / mem::size_of::<T>();
        self.acquire_mapping_raw(buffer, Some(range.clone()))
            .map(|ptr| unsafe {
                let start_ptr = ptr.offset(range.start as isize) as *const _;
                mapping::Reader {
                    slice: slice::from_raw_parts(start_ptr, count),
                    buffer,
                    released: false,
                }
            })
    }

    /// Release a mapping Reader
    ///
    /// See `acquire_mapping_raw` for more information.
    fn release_mapping_reader<'a, T>(&self, mut reader: mapping::Reader<'a, B, T>) {
        reader.released = true;
        self.release_mapping_raw(reader.buffer, None);
    }

    /// Acquire a mapping Writer
    ///
    /// The accessible slice will correspond to the specified range (in bytes).
    /// See `acquire_mapping_raw` for more information.
    fn acquire_mapping_writer<'a, T>(&self, buffer: &'a B::Buffer, range: Range<u64>)
        -> Result<mapping::Writer<'a, B, T>, mapping::Error>
    where
        T: Copy,
    {
        let count = (range.end - range.start) as usize / mem::size_of::<T>();
        self.acquire_mapping_raw(buffer, None)
            .map(|ptr| unsafe {
                let start_ptr = ptr.offset(range.start as isize) as *mut _;
                mapping::Writer {
                    slice: slice::from_raw_parts_mut(start_ptr, count),
                    buffer,
                    range,
                    released: false,
                }
            })
    }

    /// Release a mapping Writer
    ///
    /// See `acquire_mapping_raw` for more information.
    fn release_mapping_writer<'a, T>(&self, mut writer: mapping::Writer<'a, B, T>) {
        writer.released = true;
        self.release_mapping_raw(writer.buffer, Some(writer.range.clone()));
    }

    ///
    fn create_semaphore(&self) -> B::Semaphore;

    ///
    fn destroy_semaphore(&self, B::Semaphore);

    ///
    fn create_fence(&self, signaled: bool) -> B::Fence;

    ///
    fn reset_fences(&self, &[&B::Fence]);

    /// Blocks until all or one of the given fences are signaled.
    /// Returns true if fences were signaled before the timeout.
    fn wait_for_fences(&self, &[&B::Fence], WaitFor, timeout_ms: u32) -> bool;

    /// true for signaled, false for not ready
    fn get_fence_status(&self, &B::Fence) -> bool;

    ///
    fn destroy_fence(&self, B::Fence);

    ///
    fn create_query_pool(&self, ty: query::QueryType, count: u32) -> B::QueryPool;

    ///
    fn destroy_query_pool(&self, B::QueryPool);

    /// Create a new swapchain from a surface and a queue family.
    ///
    /// *Note*: The number of exposed images in the back buffer might differ
    /// from number of internally used buffers.
    ///
    /// # Safety
    ///
    /// The queue family _must_ support surface presentation.
    /// This can be checked by calling [`supports_queue_family`](trait.Surface.html#tymethod.supports_queue_family)
    /// on this surface.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate gfx_backend_empty as empty;
    /// # extern crate gfx_hal;
    /// # fn main() {
    /// use gfx_hal::{Device, SwapchainConfig};
    /// use gfx_hal::format::Format;
    /// # use gfx_hal::{CommandQueue, Graphics};
    ///
    /// # let mut surface: empty::Surface = return;
    /// # let device: empty::Device = return;
    /// let swapchain_config = SwapchainConfig::new().with_color(Format::Rgba8Srgb);
    /// device.create_swapchain(&mut surface, swapchain_config);
    /// # }
    /// ```
    fn create_swapchain(
        &self,
        surface: &mut B::Surface,
        config: SwapchainConfig,
    ) -> (B::Swapchain, Backbuffer<B>);
}
