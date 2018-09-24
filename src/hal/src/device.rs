//! Logical device
//!
//! # Device
//!
//! This module exposes the `Device` trait, which provides methods for creating
//! and managing graphics resources such as buffers, images and memory.
//!
//! The `Adapter` and `Device` types are very similar to the Vulkan concept of
//! "physical devices" vs. "logical devices"; an `Adapter` is single GPU
//! (or CPU) that implements a backend, a `Device` is a
//! handle to that physical device that has the requested capabilities
//! and is used to actually do things.

use std::any::Any;
use std::borrow::Borrow;
use std::error::Error;
use std::ops::Range;
use std::{fmt, iter, mem, slice};

use {buffer, format, image, mapping, pass, pso, query};
use {Backend, MemoryTypeId};

use error::HostExecutionError;
use memory::Requirements;
use pool::{CommandPool, CommandPoolCreateFlags};
use queue::{QueueFamilyId, QueueGroup};
use range::RangeArg;
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
    /// Requested binding to memory that doesn't support the required operations.
    WrongMemory,
    /// Requested binding to an invalid memory.
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

/// An error from creating a shader module.
#[derive(Clone, Debug, PartialEq)]
pub enum ShaderError {
    /// The shader failed to compile.
    CompilationFailed(String),
    /// Missing entry point.
    MissingEntryPoint(String),
    /// Mismatch of interface (e.g missing push constants).
    InterfaceMismatch(String),
    /// Shader stage is not supported.
    UnsupportedStage(pso::Stage),
}

impl fmt::Display for ShaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShaderError::CompilationFailed(ref msg) => {
                write!(f, "shader compilation failed: {}", msg)
            }
            ShaderError::MissingEntryPoint(ref msg) => {
                write!(f, "shader is missing an entry point: {}", msg)
            }
            ShaderError::InterfaceMismatch(ref msg) => {
                write!(f, "shader interface mismatch: {}", msg)
            }
            ShaderError::UnsupportedStage(stage) => {
                write!(f, "shader stage \"{}\" is unsupported", stage)
            }
        }
    }
}

impl Error for ShaderError {}

/// An error from creating a framebuffer.
#[derive(Clone, Debug, PartialEq)]
pub struct FramebufferError;

impl fmt::Display for FramebufferError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error creating framebuffer")
    }
}

impl Error for FramebufferError {}

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
pub trait Device<B: Backend>: Any + Send + Sync {
    /// Allocates a memory segment of a specified type.
    ///
    /// There is only a limited amount of allocations allowed depending on the implementation!
    ///
    /// # Arguments
    ///
    /// * `memory_type` - Index of the memory type in the memory properties of the associated physical device.
    /// * `size` - Size of the allocation.
    fn allocate_memory(
        &self,
        memory_type: MemoryTypeId,
        size: u64,
    ) -> Result<B::Memory, OutOfMemory>;

    /// Free device memory
    fn free_memory(&self, memory: B::Memory);

    /// Create a new command pool for a given queue family.
    ///
    /// *Note*: the family has to be associated by one as the `Gpu::queue_groups`.
    fn create_command_pool(
        &self,
        family: QueueFamilyId,
        create_flags: CommandPoolCreateFlags,
    ) -> B::CommandPool;

    /// Create a strongly typed command pool wrapper.
    fn create_command_pool_typed<C>(
        &self,
        group: &QueueGroup<B, C>,
        flags: CommandPoolCreateFlags,
        max_buffers: usize,
    ) -> CommandPool<B, C> {
        let raw = self.create_command_pool(group.family(), flags);
        let mut pool = unsafe { CommandPool::new(raw) };
        pool.reserve(max_buffers);
        pool
    }

    /// Destroy a command pool.
    fn destroy_command_pool(&self, pool: B::CommandPool);

    /// Create a render pass with the given attachments and subpasses.
    ///
    /// A *render pass* represents a collection of attachments, subpasses, and dependencies between
    /// the subpasses, and describes how the attachments are used over the course of the subpasses.
    /// The use of a render pass in a command buffer is a *render pass* instance.
    fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        subpasses: IS,
        dependencies: ID,
    ) -> B::RenderPass
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>;

    /// Destroy a `RenderPass`.
    fn destroy_render_pass(&self, rp: B::RenderPass);

    /// Create a new pipeline layout object.
    ///
    /// # Arguments
    ///
    /// * `set_layouts` - Descriptor set layouts
    /// * `push_constants` - Ranges of push constants. A shader stage may only contain one push
    ///     constant block. The length of the range indicates the number of u32 constants occupied
    ///     by the push constant block.
    ///
    /// # PipelineLayout
    ///
    /// Access to descriptor sets from a pipeline is accomplished through a *pipeline layout*.
    /// Zero or more descriptor set layouts and zero or more push constant ranges are combined to
    /// form a pipeline layout object which describes the complete set of resources that **can** be
    /// accessed by a pipeline. The pipeline layout represents a sequence of descriptor sets with
    /// each having a specific layout. This sequence of layouts is used to determine the interface
    /// between shader stages and shader resources. Each pipeline is created using a pipeline layout.
    fn create_pipeline_layout<IS, IR>(
        &self,
        set_layouts: IS,
        push_constant: IR,
    ) -> B::PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<B::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>;

    /// Destroy a pipeline layout object
    fn destroy_pipeline_layout(&self, layout: B::PipelineLayout);

    /// Create a pipeline cache object.
    //TODO: allow loading from disk
    fn create_pipeline_cache(&self) -> B::PipelineCache;

    /// Merge a number of source pipeline caches into the target one.
    fn merge_pipeline_caches<I>(&self, target: &B::PipelineCache, sources: I)
    where
        I: IntoIterator,
        I::Item: Borrow<B::PipelineCache>;

    /// Destroy a pipeline cache object.
    fn destroy_pipeline_cache(&self, cache: B::PipelineCache);

    /// Create a graphics pipeline.
    fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, B>,
        cache: Option<&B::PipelineCache>,
    ) -> Result<B::GraphicsPipeline, pso::CreationError> {
        self.create_graphics_pipelines(iter::once(desc), cache)
            .remove(0)
    }

    /// Create graphics pipelines.
    fn create_graphics_pipelines<'a, I>(
        &self,
        descs: I,
        cache: Option<&B::PipelineCache>,
    ) -> Vec<Result<B::GraphicsPipeline, pso::CreationError>>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::GraphicsPipelineDesc<'a, B>>,
    {
        descs
            .into_iter()
            .map(|desc| self.create_graphics_pipeline(desc.borrow(), cache))
            .collect()
    }

    /// Destroy a graphics pipeline.
    ///
    /// The graphics pipeline shouldn't be destroyed before any submitted command buffer,
    /// which references the graphics pipeline, has finished execution.
    fn destroy_graphics_pipeline(&self, pipeline: B::GraphicsPipeline);

    /// Create a compute pipeline.
    fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, B>,
        cache: Option<&B::PipelineCache>,
    ) -> Result<B::ComputePipeline, pso::CreationError> {
        self.create_compute_pipelines(iter::once(desc), cache)
            .remove(0)
    }

    /// Create compute pipelines.
    fn create_compute_pipelines<'a, I>(
        &self,
        descs: I,
        cache: Option<&B::PipelineCache>,
    ) -> Vec<Result<B::ComputePipeline, pso::CreationError>>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::ComputePipelineDesc<'a, B>>,
    {
        descs
            .into_iter()
            .map(|desc| self.create_compute_pipeline(desc.borrow(), cache))
            .collect()
    }

    /// Destroy a compute pipeline.
    ///
    /// The compute pipeline shouldn't be destroyed before any submitted command buffer,
    /// which references the compute pipeline, has finished execution.
    fn destroy_compute_pipeline(&self, pipeline: B::ComputePipeline);

    /// Create a new framebuffer object
    fn create_framebuffer<I>(
        &self,
        pass: &B::RenderPass,
        attachments: I,
        extent: image::Extent,
    ) -> Result<B::Framebuffer, FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<B::ImageView>;

    /// Destroy a framebuffer.
    ///
    /// The framebuffer shouldn't be destroy before any submitted command buffer,
    /// which references the framebuffer, has finished execution.
    fn destroy_framebuffer(&self, buf: B::Framebuffer);

    /// Create a new shader module object through the SPIR-V binary data.
    ///
    /// Once a shader module has been created, any entry points it contains can be used in pipeline
    /// shader stages as described in *Compute Pipelines* and *Graphics Pipelines*.
    fn create_shader_module(&self, spirv_data: &[u8]) -> Result<B::ShaderModule, ShaderError>;

    /// Destroy a shader module module
    ///
    /// A shader module can be destroyed while pipelines created using its shaders are still in use.
    fn destroy_shader_module(&self, shader: B::ShaderModule);

    /// Create a new buffer (unbound).
    ///
    /// The created buffer won't have associated memory until `bind_buffer_memory` is called.
    fn create_buffer(
        &self,
        size: u64,
        usage: buffer::Usage,
    ) -> Result<B::UnboundBuffer, buffer::CreationError>;

    /// Get memory requirements for the unbound buffer
    fn get_buffer_requirements(&self, buf: &B::UnboundBuffer) -> Requirements;

    /// Bind memory to a buffer.
    ///
    /// The unbound buffer will be consumed because the binding is *immutable*.
    /// Be sure to check that there is enough memory available for the buffer.
    /// Use `get_buffer_requirements` to acquire the memory requirements.
    fn bind_buffer_memory(
        &self,
        memory: &B::Memory,
        offset: u64,
        buf: B::UnboundBuffer,
    ) -> Result<B::Buffer, BindError>;

    /// Destroy a buffer.
    ///
    /// The buffer shouldn't be destroyed before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_buffer(&self, B::Buffer);

    /// Create a new buffer view object
    fn create_buffer_view<R: RangeArg<u64>>(
        &self,
        buf: &B::Buffer,
        fmt: Option<format::Format>,
        range: R,
    ) -> Result<B::BufferView, buffer::ViewCreationError>;

    /// Destroy a buffer view object
    fn destroy_buffer_view(&self, view: B::BufferView);

    /// Create a new image object
    fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        tiling: image::Tiling,
        usage: image::Usage,
        view_caps: image::ViewCapabilities,
    ) -> Result<B::UnboundImage, image::CreationError>;

    /// Get memory requirements for the unbound Image
    fn get_image_requirements(&self, image: &B::UnboundImage) -> Requirements;

    ///
    fn get_image_subresource_footprint(
        &self,
        image: &B::Image,
        subresource: image::Subresource,
    ) -> image::SubresourceFootprint;

    /// Bind device memory to an image object
    fn bind_image_memory(
        &self,
        &B::Memory,
        offset: u64,
        B::UnboundImage,
    ) -> Result<B::Image, BindError>;

    /// Destroy an image.
    ///
    /// The image shouldn't be destroyed before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_image(&self, image: B::Image);

    /// Create an image view from an existing image
    fn create_image_view(
        &self,
        image: &B::Image,
        view_kind: image::ViewKind,
        format: format::Format,
        swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<B::ImageView, image::ViewError>;

    /// Destroy an image view object
    fn destroy_image_view(&self, view: B::ImageView);

    /// Create a new sampler object
    fn create_sampler(&self, info: image::SamplerInfo) -> B::Sampler;

    /// Destroy a sampler object
    fn destroy_sampler(&self, sampler: B::Sampler);

    /// Create a descriptor pool.
    ///
    /// Descriptor pools allow allocation of descriptor sets.
    /// The pool can't be modified directly, only through updating descriptor sets.
    fn create_descriptor_pool<I>(&self, max_sets: usize, descriptor_ranges: I) -> B::DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>;

    /// Destroy a descriptor pool object
    ///
    /// When a pool is destroyed, all descriptor sets allocated from the pool are implicitly freed
    /// and become invalid. Descriptor sets allocated from a given pool do not need to be freed
    /// before destroying that descriptor pool.
    fn destroy_descriptor_pool(&self, pool: B::DescriptorPool);

    /// Create a descriptor set layout.
    ///
    /// A descriptor set layout object is defined by an array of zero or more descriptor bindings.
    /// Each individual descriptor binding is specified by a descriptor type, a count (array size)
    /// of the number of descriptors in the binding, a set of shader stages that **can** access the
    /// binding, and (if using immutable samplers) an array of sampler descriptors.
    fn create_descriptor_set_layout<I, J>(
        &self,
        bindings: I,
        immutable_samplers: J,
    ) -> B::DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<B::Sampler>;

    /// Destroy a descriptor set layout object
    fn destroy_descriptor_set_layout(&self, layout: B::DescriptorSetLayout);

    /// Specifying the parameters of a descriptor set write operation
    fn write_descriptor_sets<'a, I, J>(&self, write_iter: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, B, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, B>>;

    /// Structure specifying a copy descriptor set operation
    fn copy_descriptor_sets<'a, I>(&self, copy_iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, B>>;

    /// Map a memory object into application address space
    ///
    /// Call `map_memory()` to retrieve a host virtual address pointer to a region of a mappable memory object
    fn map_memory<R>(&self, memory: &B::Memory, range: R) -> Result<*mut u8, mapping::Error>
    where
        R: RangeArg<u64>;

    /// Flush mapped memory ranges
    fn flush_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a B::Memory, R)>,
        R: RangeArg<u64>;

    /// Invalidate ranges of non-coherent memory from the host caches
    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, ranges: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a B::Memory, R)>,
        R: RangeArg<u64>;

    /// Unmap a memory object once host access to it is no longer needed by the application
    fn unmap_memory(&self, memory: &B::Memory);

    /// Acquire a mapping Reader.
    ///
    /// The accessible slice will correspond to the specified range (in bytes).
    fn acquire_mapping_reader<'a, T>(
        &self,
        memory: &'a B::Memory,
        range: Range<u64>,
    ) -> Result<mapping::Reader<'a, B, T>, mapping::Error>
    where
        T: Copy,
    {
        let len = range.end - range.start;
        let count = len as usize / mem::size_of::<T>();
        self.map_memory(memory, range.clone()).map(|ptr| unsafe {
            let start_ptr = ptr as *const _;
            self.invalidate_mapped_memory_ranges(iter::once((memory, range.clone())));

            mapping::Reader {
                slice: slice::from_raw_parts(start_ptr, count),
                memory,
                released: false,
            }
        })
    }

    /// Release a mapping Reader.
    fn release_mapping_reader<'a, T>(&self, mut reader: mapping::Reader<'a, B, T>) {
        reader.released = true;
        self.unmap_memory(reader.memory);
    }

    /// Acquire a mapping Writer.
    ///
    /// The accessible slice will correspond to the specified range (in bytes).
    fn acquire_mapping_writer<'a, T>(
        &self,
        memory: &'a B::Memory,
        range: Range<u64>,
    ) -> Result<mapping::Writer<'a, B, T>, mapping::Error>
    where
        T: Copy,
    {
        let count = (range.end - range.start) as usize / mem::size_of::<T>();
        self.map_memory(memory, range.clone()).map(|ptr| unsafe {
            let start_ptr = ptr as *mut _;
            mapping::Writer {
                slice: slice::from_raw_parts_mut(start_ptr, count),
                memory,
                range,
                released: false,
            }
        })
    }

    /// Release a mapping Writer.
    fn release_mapping_writer<'a, T>(&self, mut writer: mapping::Writer<'a, B, T>) {
        writer.released = true;
        self.flush_mapped_memory_ranges(iter::once((writer.memory, writer.range.clone())));
        self.unmap_memory(writer.memory);
    }

    /// Create a new semaphore object
    fn create_semaphore(&self) -> B::Semaphore;

    /// Destroy a semaphore object
    fn destroy_semaphore(&self, semaphore: B::Semaphore);

    /// Create a new fence object
    ///
    /// Fences are a synchronization primitive that **can** be used to insert a dependency from
    /// a queue to the host. Fences have two states - signaled and unsignaled. A fence **can** be
    /// signaled as part of the execution of a *queue submission* command. Fences **can** be unsignaled
    /// on the host with *reset_fences*. Fences **can** be waited on by the host with the
    /// *wait_for_fences* command, and the current state **can** be queried with *get_fence_status*.
    fn create_fence(&self, signaled: bool) -> B::Fence;

    ///
    fn reset_fence(&self, fence: &B::Fence) {
        self.reset_fences(iter::once(fence));
    }

    ///
    fn reset_fences<I>(&self, fences: I)
    where
        I: IntoIterator,
        I::Item: Borrow<B::Fence>,
    {
        for fence in fences {
            self.reset_fence(fence.borrow());
        }
    }

    /// Blocks until the given fence is signaled.
    /// Returns true if the fence was signaled before the timeout.
    fn wait_for_fence(&self, fence: &B::Fence, timeout_ns: u64) -> bool {
        self.wait_for_fences(iter::once(fence), WaitFor::All, timeout_ns)
    }

    /// Blocks until all or one of the given fences are signaled.
    /// Returns true if fences were signaled before the timeout.
    fn wait_for_fences<I>(&self, fences: I, wait: WaitFor, timeout_ns: u64) -> bool
    where
        I: IntoIterator,
        I::Item: Borrow<B::Fence>,
    {
        use std::{thread, time};
        fn to_ns(duration: time::Duration) -> u64 {
            duration.as_secs() * 1_000_000_000 + duration.subsec_nanos() as u64
        }

        let start = time::Instant::now();
        match wait {
            WaitFor::All => {
                for fence in fences {
                    if !self.wait_for_fence(fence.borrow(), 0) {
                        let elapsed_ns = to_ns(start.elapsed());
                        if elapsed_ns > timeout_ns {
                            return false;
                        }
                        if !self.wait_for_fence(fence.borrow(), timeout_ns - elapsed_ns) {
                            return false;
                        }
                    }
                }
                true
            }
            WaitFor::Any => {
                let fences: Vec<_> = fences.into_iter().collect();
                loop {
                    for fence in &fences {
                        if self.wait_for_fence(fence.borrow(), 0) {
                            return true;
                        }
                    }
                    if to_ns(start.elapsed()) >= timeout_ns {
                        return false;
                    }
                    thread::sleep(time::Duration::from_millis(1));
                }
            }
        }
    }

    /// true for signaled, false for not ready
    fn get_fence_status(&self, fence: &B::Fence) -> bool;

    /// Destroy a fence object
    fn destroy_fence(&self, fence: B::Fence);

    /// Create a new query pool object
    ///
    /// Queries are managed using query pool objects. Each query pool is a collection of a specific
    /// number of queries of a particular type.
    fn create_query_pool(
        &self,
        ty: query::Type,
        count: query::Id,
    ) -> Result<B::QueryPool, query::Error>;

    /// Destroy a query pool object
    fn destroy_query_pool(&self, pool: B::QueryPool);

    /// Get query pool results into the specified CPU memory.
    /// Returns `Ok(false)` if the results are not ready yet and neither of `WAIT` or `PARTIAL` flags are set.
    fn get_query_pool_results(
        &self,
        pool: &B::QueryPool,
        queries: Range<query::Id>,
        data: &mut [u8],
        stride: buffer::Offset,
        flags: query::ResultFlags,
    ) -> Result<bool, query::Error>;

    /// Create a new swapchain from a surface and a queue family, optionally providing the old
    /// swapchain to aid in resource reuse and rendering continuity.
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
    /// let swapchain_config = SwapchainConfig::new(100, 100, Format::Rgba8Srgb, 2);
    /// device.create_swapchain(&mut surface, swapchain_config, None);
    /// # }
    /// ```
    fn create_swapchain(
        &self,
        surface: &mut B::Surface,
        config: SwapchainConfig,
        old_swapchain: Option<B::Swapchain>,
    ) -> (B::Swapchain, Backbuffer<B>);

    ///
    fn destroy_swapchain(&self, swapchain: B::Swapchain);

    /// Wait for all queues associated with this device to idle.
    ///
    /// Host access to all queues needs to be **externally** sycnhronized!
    fn wait_idle(&self) -> Result<(), HostExecutionError>;
}
