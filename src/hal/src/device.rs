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

use std::{fmt, mem, slice};
use std::any::Any;
use std::borrow::Borrow;
use std::error::Error;
use std::ops::Range;

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

/// Describes the size of an Image, which may be up to three dimensional.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Extent {
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// Image depth.
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
    /// Shader stage is not supported.
    UnsupportedStage(pso::Stage),
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
pub trait Device<B: Backend>: Any + Send + Sync {
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
    fn create_command_pool(&self, QueueFamilyId, CommandPoolCreateFlags) -> B::CommandPool;

    /// Creates a strongly typed command pool wrapper.
    fn create_command_pool_typed<C>(
        &self,
        group: &QueueGroup<B, C>,
        flags: CommandPoolCreateFlags,
        max_buffers: usize,
    ) -> CommandPool<B, C> {
        let raw = self.create_command_pool(group.family(), flags);
        CommandPool::new(raw, max_buffers)
    }

    /// Destroys a command pool.
    fn destroy_command_pool(&self, B::CommandPool);

    /// Creates a render pass with the given attachments and subpasses.
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

    /// Destroys a `RenderPass`.
    fn destroy_render_pass(&self, B::RenderPass);

    /// Create a new pipeline layout.
    ///
    /// # Arguments
    ///
    /// * `set_layouts` - Descriptor set layouts
    /// * `push_constants` - Ranges of push constants. A shader stage may only contain one push
    ///     constant block. The length of the range indicates the number of u32 constants occupied
    ///     by the push constant block.
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

    ///
    fn destroy_pipeline_layout(&self, B::PipelineLayout);

    /// Create a graphics pipeline.
    fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, B>
    ) -> Result<B::GraphicsPipeline, pso::CreationError> {
        self.create_graphics_pipelines(Some(desc)).remove(0)
    }

    /// Create graphics pipelines.
    fn create_graphics_pipelines<'a, I>(
        &self, descs: I
    ) -> Vec<Result<B::GraphicsPipeline, pso::CreationError>>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::GraphicsPipelineDesc<'a, B>>,
    {
        descs.into_iter().map(|desc| self.create_graphics_pipeline(desc.borrow())).collect()
    }

    /// Destroys a graphics pipeline.
    ///
    /// The graphics pipeline shouldn't be destroyed before any submitted command buffer,
    /// which references the graphics pipeline, has finished execution.
    fn destroy_graphics_pipeline(&self, B::GraphicsPipeline);

    /// Create a compute pipeline.
    fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, B>
    ) -> Result<B::ComputePipeline, pso::CreationError> {
        self.create_compute_pipelines(Some(desc)).remove(0)
    }

    /// Create compute pipelines.
    fn create_compute_pipelines<'a, I>(
        &self, descs: I
    ) -> Vec<Result<B::ComputePipeline, pso::CreationError>>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::ComputePipelineDesc<'a, B>>,
    {
        descs.into_iter().map(|desc| self.create_compute_pipeline(desc.borrow())).collect()
    }

    /// Destroys a compute pipeline.
    ///
    /// The compute pipeline shouldn't be destroyed before any submitted command buffer,
    /// which references the compute pipeline, has finished execution.
    fn destroy_compute_pipeline(&self, B::ComputePipeline);

    ///
    fn create_framebuffer<I>(
        &self,
        pass: &B::RenderPass,
        attachments: I,
        extent: Extent,
    ) -> Result<B::Framebuffer, FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<B::ImageView>;

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
    fn create_buffer_view<R: RangeArg<u64>>(
        &self, &B::Buffer, Option<format::Format>, R
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
    /// Ihe pool can't be modified directly, only through updating descriptor sets.
    fn create_descriptor_pool<I>(&self, max_sets: usize, descriptor_ranges: I) -> B::DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>;

    ///
    fn destroy_descriptor_pool(&self, B::DescriptorPool);

    /// Create a descriptor set layout.
    fn create_descriptor_set_layout<I>(&self, bindings: I) -> B::DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>;

    ///
    fn destroy_descriptor_set_layout(&self, B::DescriptorSetLayout);

    ///
    fn write_descriptor_sets<'a, I, J>(&self, I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, B, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, B>>;

    ///
    fn copy_descriptor_sets<'a, I>(&self, I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, B>>;

    ///
    fn map_memory<R>(&self, &B::Memory, R) -> Result<*mut u8, mapping::Error>
    where
        R: RangeArg<u64>;

    ///
    fn flush_mapped_memory_ranges<'a, I, R>(&self, I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a B::Memory, R)>,
        R: RangeArg<u64>;

    ///
    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a B::Memory, R)>,
        R: RangeArg<u64>;

    ///
    fn unmap_memory(&self, &B::Memory);

    /// Acquire a mapping Reader.
    ///
    /// The accessible slice will correspond to the specified range (in bytes).
    fn acquire_mapping_reader<'a, T>(&self, memory: &'a B::Memory, range: Range<u64>)
        -> Result<mapping::Reader<'a, B, T>, mapping::Error>
    where
        T: Copy,
    {
        let len = range.end - range.start;
        let count = len as usize / mem::size_of::<T>();
        self.map_memory(memory, range.clone())
            .map(|ptr| unsafe {
                let start_ptr = ptr as *const _;
                self.invalidate_mapped_memory_ranges(Some((memory, range.clone())));

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
    fn acquire_mapping_writer<'a, T>(&self, memory: &'a B::Memory, range: Range<u64>)
        -> Result<mapping::Writer<'a, B, T>, mapping::Error>
    where
        T: Copy,
    {
        let count = (range.end - range.start) as usize / mem::size_of::<T>();
        self.map_memory(memory, range.clone())
            .map(|ptr| unsafe {
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
        self.flush_mapped_memory_ranges(Some((writer.memory, writer.range.clone())));
        self.unmap_memory(writer.memory);
    }

    ///
    fn create_semaphore(&self) -> B::Semaphore;

    ///
    fn destroy_semaphore(&self, B::Semaphore);

    ///
    fn create_fence(&self, signaled: bool) -> B::Fence;

    ///
    fn reset_fence(&self, fence: &B::Fence) {
        self.reset_fences(Some(fence));
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
    fn wait_for_fence(&self, fence: &B::Fence, timeout_ms: u32) -> bool {
        self.wait_for_fences(Some(fence), WaitFor::All, timeout_ms)
    }

    /// Blocks until all or one of the given fences are signaled.
    /// Returns true if fences were signaled before the timeout.
    fn wait_for_fences<I>(&self, fences: I, wait: WaitFor, timeout_ms: u32) -> bool
    where
        I: IntoIterator,
        I::Item: Borrow<B::Fence>,
    {
        use std::{time, thread};
        let start = time::Instant::now();
        fn to_ms(duration: time::Duration) -> u32 {
            duration.as_secs() as u32 * 1000 + duration.subsec_nanos() / 1_000_000
        }
        match wait {
            WaitFor::All => {
                for fence in fences {
                    if !self.wait_for_fence(fence.borrow(), 0) {
                        let elapsed_ms = to_ms(start.elapsed());
                        if elapsed_ms > timeout_ms {
                            return false;
                        }
                        if !self.wait_for_fence(fence.borrow(), timeout_ms - elapsed_ms) {
                            return false;
                        }
                    }
                }
                true
            },
            WaitFor::Any => {
                let fences: Vec<_> = fences.into_iter().collect();
                loop {
                    for fence in &fences {
                        if self.wait_for_fence(fence.borrow(), 0) {
                            return true;
                        }
                    }
                    if to_ms(start.elapsed()) >= timeout_ms {
                        return false;
                    }
                    thread::sleep(time::Duration::from_millis(1));
                }
            }
        }
    }

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

    /// Wait for all queues associated with this device to idle.
    ///
    /// Host access to all queues needs to be **externally** sycnhronized!
    fn wait_idle(&self) -> Result<(), HostExecutionError>;
}
