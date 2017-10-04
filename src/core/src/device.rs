//! # Device
//!
//! This module exposes the `Device` trait, used for creating and managing graphics resources, and
//! includes several items to facilitate this.

use std::{fmt, mem, slice};
use std::error::Error;
use std::ops::Range;
use {buffer, format, image, mapping, pass, pso};
use {Backend, Features, Limits, MemoryType};
use memory::Requirements;


/// Error allocating memory.
#[derive(Clone, PartialEq, Debug)]
pub struct OutOfMemory;

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
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum WaitFor {
    /// Wait for any target.
    Any,
    /// Wait for all targets at once.
    All,
}

///
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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
/// This trait is extended by the [`gfx::DeviceExt` trait](https://docs.rs/gfx/*/gfx/traits/trait.DeviceExt.html).
/// All types implementing `Device` also implement `DeviceExt`.
///
///
/// ## Raw resources
///
/// The term "raw" is used in the context of types of functions that have a strongly typed and an
/// untyped equivalent, to refer to the untyped equivalent.
///
/// For example ['Device::create_buffer_raw'](trait.Device.html#tymethod.create_buffer_raw) and
/// ['Device::create_buffer'](trait.Device.html#tymethod.create_buffer)
///
/// ## Shader resource views and unordered access views
///
/// This terminology is borrowed from D3D.
///
/// Shader resource views typically wrap textures and buffers to provide read-only access in shaders.
/// An unordered access view provides similar functionality, but enables reading and writing to
/// the buffer or texture in any order.
///
/// See:
///
/// - [The gfx::UNORDERED_ACCESS bit in the gfx::Bind flags](../gfx/struct.Bind.html).
/// - [Device::view_buffer_as_unordered_access](trait.Device.html#method.view_buffer_as_unordered_access).
///
#[allow(missing_docs)]
pub trait Device<B: Backend>: Clone {
    /// Returns the features of this `Device`. This usually depends on the graphics API being
    /// used.
    fn get_features(&self) -> &Features;

    /// Returns the limits of this `Device`.
    fn get_limits(&self) -> &Limits;

    /// Allocate a memory segment of a specified type.
    ///
    /// There is only a limited amount of allocations allowed depending on the implementation!
    fn allocate_memory(&mut self, &MemoryType, size: u64) -> Result<B::Memory, OutOfMemory>;

    ///
    fn create_renderpass(
        &mut self,
        &[pass::Attachment],
        &[pass::SubpassDesc],
        &[pass::SubpassDependency],
    ) -> B::RenderPass;

    ///
    fn create_pipeline_layout(
        &mut self,
        &[&B::DescriptorSetLayout],
    ) -> B::PipelineLayout;

    /// Create graphics pipelines.
    fn create_graphics_pipelines<'a>(
        &mut self,
        &[(pso::GraphicsShaderSet<'a, B>, &B::PipelineLayout, pass::Subpass<'a, B>, &pso::GraphicsPipelineDesc)],
    ) -> Vec<Result<B::GraphicsPipeline, pso::CreationError>>;

    /// Create compute pipelines.
    fn create_compute_pipelines<'a>(
        &mut self,
        &[(pso::EntryPoint<'a, B>, &B::PipelineLayout)],
    ) -> Vec<Result<B::ComputePipeline, pso::CreationError>>;

    ///
    fn create_framebuffer(
        &mut self,
        &B::RenderPass,
        &[&B::ImageView],
        Extent,
    ) -> Result<B::Framebuffer, FramebufferError>;

    ///
    fn create_shader_module(&mut self, spirv_data: &[u8]) -> Result<B::ShaderModule, ShaderError>;

    /// Create a new buffer (unbound).
    ///
    /// The created buffer won't have associated memory until `bind_buffer_memory` is called.
    fn create_buffer(&mut self, size: u64, stride: u64, buffer::Usage) -> Result<B::UnboundBuffer, buffer::CreationError>;

    ///
    fn get_buffer_requirements(&mut self, &B::UnboundBuffer) -> Requirements;

    /// Bind memory to a buffer.
    ///
    /// The unbound buffer will be consumed because the binding is *immutable*.
    /// Be sure to check that there is enough memory available for the buffer.
    /// Use `get_buffer_requirements` to acquire the memory requirements.
    fn bind_buffer_memory(&mut self, &B::Memory, offset: u64, B::UnboundBuffer) -> Result<B::Buffer, BindError>;

    ///
    fn create_buffer_view(&mut self, &B::Buffer, Range<u64>) -> Result<B::BufferView, buffer::ViewError>;

    ///
    fn create_image(&mut self, image::Kind, image::Level, format::Format, image::Usage) -> Result<B::UnboundImage, image::CreationError>;

    ///
    fn get_image_requirements(&mut self, &B::UnboundImage) -> Requirements;

    ///
    fn bind_image_memory(&mut self, &B::Memory, offset: u64, B::UnboundImage) -> Result<B::Image, BindError>;

    ///
    fn create_image_view(&mut self, &B::Image, format::Format, image::SubresourceLayers) -> Result<B::ImageView, image::ViewError>;

    ///
    fn create_sampler(&mut self, image::SamplerInfo) -> B::Sampler;

    /// Create a descriptor pool.
    ///
    /// Descriptor pools allow allocation of descriptor sets.
    /// The pool can't be modified directly, only trough updating descriptor sets.
    fn create_descriptor_pool(
        &mut self,
        max_sets: usize,
        &[pso::DescriptorRangeDesc],
    ) -> B::DescriptorPool;

    /// Create a descriptor set layout.
    fn create_descriptor_set_layout(
        &mut self,
        &[pso::DescriptorSetLayoutBinding],
    ) -> B::DescriptorSetLayout;

    ///
    // TODO: copies
    fn update_descriptor_sets(&mut self, &[pso::DescriptorSetWrite<B>]);

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
    fn acquire_mapping_raw(&mut self, buf: &B::Buffer, read: Option<Range<u64>>)
        -> Result<*mut u8, mapping::Error>;

    /// Release access to the buffer mapping.
    ///
    /// If you wrote, you have to specify in which range.
    fn release_mapping_raw(&mut self, buf: &B::Buffer, wrote: Option<Range<u64>>);

    /// Acquire a mapping Reader
    ///
    /// The accessible slice will correspond to the specified range (in bytes).
    /// See `acquire_mapping_raw` for more information.
    fn acquire_mapping_reader<'a, T>(&mut self, buffer: &'a B::Buffer, range: Range<u64>)
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
    fn release_mapping_reader<'a, T>(&mut self, mut reader: mapping::Reader<'a, B, T>) {
        reader.released = true;
        self.release_mapping_raw(reader.buffer, None);
    }

    /// Acquire a mapping Writer
    ///
    /// The accessible slice will correspond to the specified range (in bytes).
    /// See `acquire_mapping_raw` for more information.
    fn acquire_mapping_writer<'a, T>(&mut self, buffer: &'a B::Buffer, range: Range<u64>)
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

    fn release_mapping_writer<'a, T>(&mut self, mut writer: mapping::Writer<'a, B, T>) {
        writer.released = true;
        self.release_mapping_raw(writer.buffer, Some(writer.range.clone()));
    }

    ///
    fn create_semaphore(&mut self) -> B::Semaphore;

    ///
    fn create_fence(&mut self, signaled: bool) -> B::Fence;

    ///
    fn reset_fences(&mut self, &[&B::Fence]);

    /// Blocks until all or one of the given fences are signaled.
    /// Returns true if fences were signaled before the timeout.
    fn wait_for_fences(&mut self, &[&B::Fence], WaitFor, timeout_ms: u32) -> bool;

    ///
    fn free_memory(&mut self, B::Memory);

    ///
    fn destroy_shader_module(&mut self, B::ShaderModule);

    ///
    fn destroy_renderpass(&mut self, B::RenderPass);

    ///
    fn destroy_pipeline_layout(&mut self, B::PipelineLayout);

    /// Destroys a graphics pipeline.
    ///
    /// The graphics pipeline shouldn't be destroy before any submitted command buffer,
    /// which references the graphics pipeline, has finished execution.
    fn destroy_graphics_pipeline(&mut self, B::GraphicsPipeline);

    /// Destroys a compute pipeline.
    ///
    /// The compute pipeline shouldn't be destroy before any submitted command buffer,
    /// which references the compute pipeline, has finished execution.
    fn destroy_compute_pipeline(&mut self, B::ComputePipeline);

    /// Destroys a framebuffer.
    ///
    /// The framebuffer shouldn't be destroy before any submitted command buffer,
    /// which references the framebuffer, has finished execution.
    fn destroy_framebuffer(&mut self, B::Framebuffer);

    /// Destroys a buffer.
    ///
    /// The buffer shouldn't be destroy before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_buffer(&mut self, B::Buffer);

    ///
    fn destroy_buffer_view(&mut self, B::BufferView);


    /// Destroys an image.
    ///
    /// The image shouldn't be destroy before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_image(&mut self, B::Image);

    ///
    fn destroy_image_view(&mut self, B::ImageView);

    ///
    fn destroy_sampler(&mut self, B::Sampler);

    ///
    fn destroy_descriptor_pool(&mut self, B::DescriptorPool);

    ///
    fn destroy_descriptor_set_layout(&mut self, B::DescriptorSetLayout);

    ///
    fn destroy_fence(&mut self, B::Fence);

    ///
    fn destroy_semaphore(&mut self, B::Semaphore);
}
