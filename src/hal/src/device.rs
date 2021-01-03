//! Logical graphics device.
//!
//! # Device
//!
//! This module exposes the [`Device`][Device] trait, which provides methods for creating
//! and managing graphics resources such as buffers, images and memory.
//!
//! The `Adapter` and `Device` types are very similar to the Vulkan concept of
//! "physical devices" vs. "logical devices"; an `Adapter` is single GPU
//! (or CPU) that implements a backend, a `Device` is a
//! handle to that physical device that has the requested capabilities
//! and is used to actually do things.

use crate::{
    buffer, format, image,
    memory::{Requirements, Segment},
    pass,
    pool::CommandPoolCreateFlags,
    pso,
    pso::DescriptorPoolCreateFlags,
    query,
    queue::QueueFamilyId,
    Backend, MemoryTypeId,
};

use std::{
    any::Any,
    borrow::{Borrow, BorrowMut},
    fmt, iter,
    ops::Range,
};

/// Error occurred caused device to be lost.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("Device lost")]
pub struct DeviceLost;

/// Error allocating memory.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum OutOfMemory {
    /// Host memory exhausted.
    #[error("Out of host memory")]
    Host,
    /// Device memory exhausted.
    #[error("Out of device memory")]
    Device,
}

/// Error occurring when waiting for fences or events.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum WaitError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),
    /// Device is lost
    #[error(transparent)]
    DeviceLost(#[from] DeviceLost),
}

/// Possible cause of allocation failure.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum AllocationError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),

    /// Cannot create any more objects.
    #[error("Too many objects")]
    TooManyObjects,
}

/// Device creation errors during `open`.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CreationError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),
    /// Device initialization failed due to implementation specific errors.
    #[error("Implementation specific error occurred")]
    InitializationFailed,
    /// At least one of the user requested extensions if not supported by the
    /// physical device.
    #[error("Requested extension is missing")]
    MissingExtension,
    /// At least one of the user requested features if not supported by the
    /// physical device.
    ///
    /// Use [`features`](trait.PhysicalDevice.html#tymethod.features)
    /// for checking the supported features.
    #[error("Requested feature is missing")]
    MissingFeature,
    /// Too many logical devices have been created from this physical device.
    ///
    /// The implementation may only support one logical device for each physical
    /// device or lacks resources to allocate a new device.
    #[error("Too many objects")]
    TooManyObjects,
    /// The logical or physical device are lost during the device creation
    /// process.
    ///
    /// This may be caused by hardware failure, physical device removal,
    /// power outage, etc.
    #[error("Logical or Physical device was lost during creation")]
    DeviceLost,
}

/// Error accessing a mapping.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum MapError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),
    /// The requested mapping range is outside of the resource.
    #[error("Requested range is outside the resource")]
    OutOfBounds,
    /// Failed to allocate an appropriately sized contiguous virtual address range.
    #[error("Unable to allocate an appropriately sized contiguous virtual address range")]
    MappingFailed,
    /// Memory is not CPU visible.
    #[error("Memory is not CPU visible")]
    Access,
}

/// Error binding a resource to memory allocation.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum BindError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),
    /// Requested binding to memory that doesn't support the required operations.
    #[error("Wrong memory")]
    WrongMemory,
    /// Requested binding to an invalid memory.
    #[error("Requested range is outside the resource")]
    OutOfBounds,
}

/// Specifies the waiting targets.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WaitFor {
    /// Wait for any target.
    Any,
    /// Wait for all targets at once.
    All,
}

/// An error from creating a shader module.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ShaderError {
    /// Unsupported module.
    #[error("Shader module is not supported")]
    Unsupported,
    /// Compilation failed.
    #[error("Shader module failed to compile: {0:}")]
    CompilationFailed(String),
    /// Device ran out of memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),
}

/// Source shader code for a module.
#[derive(Debug)]
#[non_exhaustive]
pub enum ShaderModuleDesc<'a> {
    /// SPIR-V word array.
    SpirV(&'a [u32]),
}

/// Logical device handle, responsible for creating and managing resources
/// for the physical device it was created from.
///
/// ## Resource Construction and Handling
///
/// This device structure can then be used to create and manage different resources,
/// like [buffers][Device::create_buffer], [shader modules][Device::create_shader_module]
/// and [images][Device::create_image]. See the individual methods for more information.
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
pub trait Device<B: Backend>: fmt::Debug + Any + Send + Sync {
    /// Allocates a memory segment of a specified type.
    ///
    /// There is only a limited amount of allocations allowed depending on the implementation!
    ///
    /// # Arguments
    ///
    /// * `memory_type` - Index of the memory type in the memory properties of the associated physical device.
    /// * `size` - Size of the allocation.
    unsafe fn allocate_memory(
        &self,
        memory_type: MemoryTypeId,
        size: u64,
    ) -> Result<B::Memory, AllocationError>;

    /// Free device memory
    unsafe fn free_memory(&self, memory: B::Memory);

    /// Create a new [command pool][crate::pool::CommandPool] for a given queue family.
    ///
    /// *Note*: the family has to be associated with one of [the queue groups
    /// of this device][crate::adapter::Gpu::queue_groups].
    unsafe fn create_command_pool(
        &self,
        family: QueueFamilyId,
        create_flags: CommandPoolCreateFlags,
    ) -> Result<B::CommandPool, OutOfMemory>;

    /// Destroy a command pool.
    unsafe fn destroy_command_pool(&self, pool: B::CommandPool);

    /// Create a [render pass][crate::pass] with the given attachments and subpasses.
    ///
    /// The use of a render pass in a command buffer is a *render pass* instance.
    ///
    /// # Arguments
    ///
    /// * `attachments` - [image attachments][crate::pass::Attachment] to be used in
    ///   this render pass. Usually you need at least one attachment, to be used as output.
    /// * `subpasses` - [subpasses][crate::pass::SubpassDesc] to use.
    ///   You need to use at least one subpass.
    /// * `dependencies` - [dependencies between subpasses][crate::pass::SubpassDependency].
    ///   Can be empty.
    unsafe fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        subpasses: IS,
        dependencies: ID,
    ) -> Result<B::RenderPass, OutOfMemory>
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IA::IntoIter: ExactSizeIterator,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        IS::IntoIter: ExactSizeIterator,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
        ID::IntoIter: ExactSizeIterator;

    /// Destroys a *render pass* created by this device.
    unsafe fn destroy_render_pass(&self, rp: B::RenderPass);

    /// Create a new pipeline layout object.
    ///
    /// # Arguments
    ///
    /// * `set_layouts` - Descriptor set layouts
    /// * `push_constants` - Ranges of push constants. A shader stage may only contain one push
    ///     constant block. The range is defined in units of bytes.
    ///
    /// # PipelineLayout
    ///
    /// Access to descriptor sets from a pipeline is accomplished through a *pipeline layout*.
    /// Zero or more descriptor set layouts and zero or more push constant ranges are combined to
    /// form a pipeline layout object which describes the complete set of resources that **can** be
    /// accessed by a pipeline. The pipeline layout represents a sequence of descriptor sets with
    /// each having a specific layout. This sequence of layouts is used to determine the interface
    /// between shader stages and shader resources. Each pipeline is created using a pipeline layout.
    unsafe fn create_pipeline_layout<IS, IR>(
        &self,
        set_layouts: IS,
        push_constant: IR,
    ) -> Result<B::PipelineLayout, OutOfMemory>
    where
        IS: IntoIterator,
        IS::Item: Borrow<B::DescriptorSetLayout>,
        IS::IntoIter: ExactSizeIterator,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>,
        IR::IntoIter: ExactSizeIterator;

    /// Destroy a pipeline layout object
    unsafe fn destroy_pipeline_layout(&self, layout: B::PipelineLayout);

    /// Create a pipeline cache object.
    unsafe fn create_pipeline_cache(
        &self,
        data: Option<&[u8]>,
    ) -> Result<B::PipelineCache, OutOfMemory>;

    /// Retrieve data from pipeline cache object.
    unsafe fn get_pipeline_cache_data(
        &self,
        cache: &B::PipelineCache,
    ) -> Result<Vec<u8>, OutOfMemory>;

    /// Merge a number of source pipeline caches into the target one.
    unsafe fn merge_pipeline_caches<I>(
        &self,
        target: &B::PipelineCache,
        sources: I,
    ) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<B::PipelineCache>,
        I::IntoIter: ExactSizeIterator;

    /// Destroy a pipeline cache object.
    unsafe fn destroy_pipeline_cache(&self, cache: B::PipelineCache);

    /// Create a graphics pipeline.
    ///
    /// # Arguments
    ///
    /// * `desc` - the [description][crate::pso::GraphicsPipelineDesc] of
    ///   the graphics pipeline to create.
    /// * `cache` - the pipeline cache,
    ///   [obtained from this device][Device::create_pipeline_cache],
    ///   used for faster PSO creation.
    unsafe fn create_graphics_pipeline<'a>(
        &self,
        desc: &pso::GraphicsPipelineDesc<'a, B>,
        cache: Option<&B::PipelineCache>,
    ) -> Result<B::GraphicsPipeline, pso::CreationError>;

    /// Create multiple graphics pipelines.
    unsafe fn create_graphics_pipelines<'a, I>(
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
    unsafe fn destroy_graphics_pipeline(&self, pipeline: B::GraphicsPipeline);

    /// Create a compute pipeline.
    unsafe fn create_compute_pipeline<'a>(
        &self,
        desc: &pso::ComputePipelineDesc<'a, B>,
        cache: Option<&B::PipelineCache>,
    ) -> Result<B::ComputePipeline, pso::CreationError>;

    /// Create compute pipelines.
    unsafe fn create_compute_pipelines<'a, I>(
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
    unsafe fn destroy_compute_pipeline(&self, pipeline: B::ComputePipeline);

    /// Create a new shader module object from the SPIR-V binary data.
    ///
    /// Once a shader module has been created, any [entry points][crate::pso::EntryPoint]
    /// it contains can be used in pipeline shader stages of
    /// [compute pipelines][crate::pso::ComputePipelineDesc] and
    /// [graphics pipelines][crate::pso::GraphicsPipelineDesc].
    unsafe fn create_shader_module(&self, spirv: &[u32]) -> Result<B::ShaderModule, ShaderError>;

    /// Create a new shader module from the `naga` module.
    unsafe fn create_shader_module_from_naga(
        &self,
        module: naga::Module,
    ) -> Result<B::ShaderModule, (ShaderError, naga::Module)> {
        Err((ShaderError::Unsupported, module))
    }

    /// Destroy a shader module module
    ///
    /// A shader module can be destroyed while pipelines created using its shaders are still in use.
    unsafe fn destroy_shader_module(&self, shader: B::ShaderModule);

    /// Create a new buffer (unbound).
    ///
    /// The created buffer won't have associated memory until `bind_buffer_memory` is called.
    unsafe fn create_buffer(
        &self,
        size: u64,
        usage: buffer::Usage,
    ) -> Result<B::Buffer, buffer::CreationError>;

    /// Get memory requirements for the buffer
    unsafe fn get_buffer_requirements(&self, buf: &B::Buffer) -> Requirements;

    /// Bind memory to a buffer.
    ///
    /// Be sure to check that there is enough memory available for the buffer.
    /// Use `get_buffer_requirements` to acquire the memory requirements.
    unsafe fn bind_buffer_memory(
        &self,
        memory: &B::Memory,
        offset: u64,
        buf: &mut B::Buffer,
    ) -> Result<(), BindError>;

    /// Destroy a buffer.
    ///
    /// The buffer shouldn't be destroyed before any submitted command buffer,
    /// which references the images, has finished execution.
    unsafe fn destroy_buffer(&self, buffer: B::Buffer);

    /// Create a new buffer view object
    unsafe fn create_buffer_view(
        &self,
        buf: &B::Buffer,
        fmt: Option<format::Format>,
        range: buffer::SubRange,
    ) -> Result<B::BufferView, buffer::ViewCreationError>;

    /// Destroy a buffer view object
    unsafe fn destroy_buffer_view(&self, view: B::BufferView);

    /// Create a new image object
    unsafe fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        tiling: image::Tiling,
        usage: image::Usage,
        view_caps: image::ViewCapabilities,
    ) -> Result<B::Image, image::CreationError>;

    /// Get memory requirements for the Image
    unsafe fn get_image_requirements(&self, image: &B::Image) -> Requirements;

    ///
    unsafe fn get_image_subresource_footprint(
        &self,
        image: &B::Image,
        subresource: image::Subresource,
    ) -> image::SubresourceFootprint;

    /// Bind device memory to an image object
    unsafe fn bind_image_memory(
        &self,
        memory: &B::Memory,
        offset: u64,
        image: &mut B::Image,
    ) -> Result<(), BindError>;

    /// Destroy an image.
    ///
    /// The image shouldn't be destroyed before any submitted command buffer,
    /// which references the images, has finished execution.
    unsafe fn destroy_image(&self, image: B::Image);

    /// Create an image view from an existing image
    unsafe fn create_image_view(
        &self,
        image: &B::Image,
        view_kind: image::ViewKind,
        format: format::Format,
        swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<B::ImageView, image::ViewCreationError>;

    /// Destroy an image view object
    unsafe fn destroy_image_view(&self, view: B::ImageView);

    /// Create a new sampler object
    unsafe fn create_sampler(
        &self,
        desc: &image::SamplerDesc,
    ) -> Result<B::Sampler, AllocationError>;

    /// Destroy a sampler object
    unsafe fn destroy_sampler(&self, sampler: B::Sampler);

    /// Create a descriptor pool.
    ///
    /// Descriptor pools allow allocation of descriptor sets.
    /// The pool can't be modified directly, only through updating descriptor sets.
    unsafe fn create_descriptor_pool<I>(
        &self,
        max_sets: usize,
        descriptor_ranges: I,
        flags: DescriptorPoolCreateFlags,
    ) -> Result<B::DescriptorPool, OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>,
        I::IntoIter: ExactSizeIterator;

    /// Destroy a descriptor pool object
    ///
    /// When a pool is destroyed, all descriptor sets allocated from the pool are implicitly freed
    /// and become invalid. Descriptor sets allocated from a given pool do not need to be freed
    /// before destroying that descriptor pool.
    unsafe fn destroy_descriptor_pool(&self, pool: B::DescriptorPool);

    /// Create a descriptor set layout.
    ///
    /// A descriptor set layout object is defined by an array of zero or more descriptor bindings.
    /// Each individual descriptor binding is specified by a descriptor type, a count (array size)
    /// of the number of descriptors in the binding, a set of shader stages that **can** access the
    /// binding, and (if using immutable samplers) an array of sampler descriptors.
    unsafe fn create_descriptor_set_layout<I, J>(
        &self,
        bindings: I,
        immutable_samplers: J,
    ) -> Result<B::DescriptorSetLayout, OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<B::Sampler>,
        J::IntoIter: ExactSizeIterator;

    /// Destroy a descriptor set layout object
    unsafe fn destroy_descriptor_set_layout(&self, layout: B::DescriptorSetLayout);

    /// Specifying the parameters of a descriptor set write operation.
    unsafe fn write_descriptor_set<'a, I>(&self, op: pso::DescriptorSetWrite<'a, B, I>)
    where
        I: IntoIterator,
        I::IntoIter: ExactSizeIterator,
        I::Item: Borrow<pso::Descriptor<'a, B>>;

    /// Structure specifying a copy descriptor set operation.
    unsafe fn copy_descriptor_set<'a>(&self, op: pso::DescriptorSetCopy<'a, B>);

    /// Map a memory object into application address space
    ///
    /// Call `map_memory()` to retrieve a host virtual address pointer to a region of a mappable memory object
    unsafe fn map_memory(
        &self,
        memory: &mut B::Memory,
        segment: Segment,
    ) -> Result<*mut u8, MapError>;

    /// Flush mapped memory ranges
    unsafe fn flush_mapped_memory_ranges<'a, I>(&self, ranges: I) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a B::Memory, Segment)>;

    /// Invalidate ranges of non-coherent memory from the host caches
    unsafe fn invalidate_mapped_memory_ranges<'a, I>(&self, ranges: I) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a B::Memory, Segment)>;

    /// Unmap a memory object once host access to it is no longer needed by the application
    unsafe fn unmap_memory(&self, memory: &mut B::Memory);

    /// Create a new semaphore object.
    fn create_semaphore(&self) -> Result<B::Semaphore, OutOfMemory>;

    /// Destroy a semaphore object.
    unsafe fn destroy_semaphore(&self, semaphore: B::Semaphore);

    /// Create a new fence object.
    ///
    /// Fences are a synchronization primitive that **can** be used to insert a dependency from
    /// a queue to the host.
    /// Fences have two states - signaled and unsignaled.
    ///
    /// A fence **can** be signaled as part of the execution of a
    /// [queue submission][crate::queue::CommandQueue::submit] command.
    ///
    /// Fences **can** be unsignaled on the host with
    /// [`reset_fences`][Device::reset_fences].
    ///
    /// Fences **can** be waited on by the host with the
    /// [`wait_for_fences`][Device::wait_for_fences] command.
    ///
    /// A fence's current state **can** be queried with
    /// [`get_fence_status`][Device::get_fence_status].
    ///
    /// # Arguments
    ///
    /// * `signaled` - the fence will be in its signaled state.
    fn create_fence(&self, signaled: bool) -> Result<B::Fence, OutOfMemory>;

    /// Resets a given fence to its original, unsignaled state.
    unsafe fn reset_fence(&self, fence: &mut B::Fence) -> Result<(), OutOfMemory> {
        self.reset_fences(iter::once(fence))
    }

    /// Resets multiple fences to their original states.
    unsafe fn reset_fences<I>(&self, fences: I) -> Result<(), OutOfMemory>
    where
        I: IntoIterator,
        I::Item: BorrowMut<B::Fence>,
        I::IntoIter: ExactSizeIterator,
    {
        for mut fence in fences {
            self.reset_fence(fence.borrow_mut())?;
        }
        Ok(())
    }

    /// Blocks until the given fence is signaled.
    /// Returns true if the fence was signaled before the timeout.
    unsafe fn wait_for_fence(&self, fence: &B::Fence, timeout_ns: u64) -> Result<bool, WaitError> {
        self.wait_for_fences(iter::once(fence), WaitFor::All, timeout_ns)
    }

    /// Blocks until all or one of the given fences are signaled.
    /// Returns true if fences were signaled before the timeout.
    unsafe fn wait_for_fences<I>(
        &self,
        fences: I,
        wait: WaitFor,
        timeout_ns: u64,
    ) -> Result<bool, WaitError>
    where
        I: IntoIterator,
        I::Item: Borrow<B::Fence>,
        I::IntoIter: ExactSizeIterator,
    {
        use std::{thread, time};
        fn to_ns(duration: time::Duration) -> u64 {
            duration.as_secs() * 1_000_000_000 + duration.subsec_nanos() as u64
        }

        let start = time::Instant::now();
        match wait {
            WaitFor::All => {
                for fence in fences {
                    if !self.wait_for_fence(fence.borrow(), 0)? {
                        let elapsed_ns = to_ns(start.elapsed());
                        if elapsed_ns > timeout_ns {
                            return Ok(false);
                        }
                        if !self.wait_for_fence(fence.borrow(), timeout_ns - elapsed_ns)? {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
            WaitFor::Any => {
                let fences: Vec<_> = fences.into_iter().collect();
                loop {
                    for fence in &fences {
                        if self.wait_for_fence(fence.borrow(), 0)? {
                            return Ok(true);
                        }
                    }
                    if to_ns(start.elapsed()) >= timeout_ns {
                        return Ok(false);
                    }
                    thread::sleep(time::Duration::from_millis(1));
                }
            }
        }
    }

    /// true for signaled, false for not ready
    unsafe fn get_fence_status(&self, fence: &B::Fence) -> Result<bool, DeviceLost>;

    /// Destroy a fence object
    unsafe fn destroy_fence(&self, fence: B::Fence);

    /// Create an event object.
    fn create_event(&self) -> Result<B::Event, OutOfMemory>;

    /// Destroy an event object.
    unsafe fn destroy_event(&self, event: B::Event);

    /// Query the status of an event.
    ///
    /// Returns `true` if the event is set, or `false` if it is reset.
    unsafe fn get_event_status(&self, event: &B::Event) -> Result<bool, WaitError>;

    /// Sets an event.
    unsafe fn set_event(&self, event: &mut B::Event) -> Result<(), OutOfMemory>;

    /// Resets an event.
    unsafe fn reset_event(&self, event: &mut B::Event) -> Result<(), OutOfMemory>;

    /// Create a new query pool object
    ///
    /// Queries are managed using query pool objects. Each query pool is a collection of a specific
    /// number of queries of a particular type.
    unsafe fn create_query_pool(
        &self,
        ty: query::Type,
        count: query::Id,
    ) -> Result<B::QueryPool, query::CreationError>;

    /// Destroy a query pool object
    unsafe fn destroy_query_pool(&self, pool: B::QueryPool);

    /// Get query pool results into the specified CPU memory.
    /// Returns `Ok(false)` if the results are not ready yet and neither of `WAIT` or `PARTIAL` flags are set.
    unsafe fn get_query_pool_results(
        &self,
        pool: &B::QueryPool,
        queries: Range<query::Id>,
        data: &mut [u8],
        stride: buffer::Stride,
        flags: query::ResultFlags,
    ) -> Result<bool, WaitError>;

    /// Wait for all queues associated with this device to idle.
    ///
    /// Host access to all queues needs to be **externally** sycnhronized!
    fn wait_idle(&self) -> Result<(), OutOfMemory>;

    /// Associate a name with an image, for easier debugging in external tools or with validation
    /// layers that can print a friendly name when referring to objects in error messages
    unsafe fn set_image_name(&self, image: &mut B::Image, name: &str);
    /// Associate a name with a buffer, for easier debugging in external tools or with validation
    /// layers that can print a friendly name when referring to objects in error messages
    unsafe fn set_buffer_name(&self, buffer: &mut B::Buffer, name: &str);
    /// Associate a name with a command buffer, for easier debugging in external tools or with
    /// validation layers that can print a friendly name when referring to objects in error messages
    unsafe fn set_command_buffer_name(&self, command_buffer: &mut B::CommandBuffer, name: &str);
    /// Associate a name with a semaphore, for easier debugging in external tools or with validation
    /// layers that can print a friendly name when referring to objects in error messages
    unsafe fn set_semaphore_name(&self, semaphore: &mut B::Semaphore, name: &str);
    /// Associate a name with a fence, for easier debugging in external tools or with validation
    /// layers that can print a friendly name when referring to objects in error messages
    unsafe fn set_fence_name(&self, fence: &mut B::Fence, name: &str);
    /// Associate a name with a render pass, for easier debugging in external tools or with
    /// validation layers that can print a friendly name when referring to objects in error messages
    unsafe fn set_render_pass_name(&self, render_pass: &mut B::RenderPass, name: &str);
    /// Associate a name with a descriptor set, for easier debugging in external tools or with
    /// validation layers that can print a friendly name when referring to objects in error messages
    unsafe fn set_descriptor_set_name(&self, descriptor_set: &mut B::DescriptorSet, name: &str);
    /// Associate a name with a descriptor set layout, for easier debugging in external tools or
    /// with validation layers that can print a friendly name when referring to objects in error
    /// messages
    unsafe fn set_descriptor_set_layout_name(
        &self,
        descriptor_set_layout: &mut B::DescriptorSetLayout,
        name: &str,
    );
    /// Associate a name with a pipeline layout, for easier debugging in external tools or with
    /// validation layers that can print a friendly name when referring to objects in error messages
    unsafe fn set_pipeline_layout_name(&self, pipeline_layout: &mut B::PipelineLayout, name: &str);
}
