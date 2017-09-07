//! # Device
//!
//! This module exposes the `Device` trait, used for creating and managing graphics resources, and
//! includes several items to facilitate this.

use std::{fmt, mem, slice};
use std::error::Error;
use std::ops::Range;
use {buffer, format, image, mapping, pass, pso, target};
use {Backend, Features, HeapType, Limits};
use memory::Requirements;


/// Type of the resources that can be allocated on a heap.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub enum ResourceHeapType {
    ///
    Any,
    ///
    Buffers,
    ///
    Images,
    ///
    Targets,
}

/// Error creating a resource heap.
#[derive(Clone, PartialEq, Debug)]
pub enum ResourceHeapError {
    /// Requested `ResourceHeapType::Any` is not supported.
    UnsupportedType,
    /// Unable to allocate the specified size.
    OutOfMemory,
}

/// Error creating either a ShaderResourceView, or UnorderedAccessView.
#[derive(Clone, Debug, PartialEq)]
pub enum ResourceViewError {
    /// The corresponding bind flag is not present in the texture.
    NoBindFlag,
    /// Selected channel type is not supported for this texture.
    Channel(format::ChannelType),
    /// Selected layer can not be viewed for this texture.
    Layer(image::LayerError),
    /// The backend was refused for some reason.
    Unsupported,
}

impl fmt::Display for ResourceViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ResourceViewError::Channel(ref channel_type) => write!(f, "{}: {:?}", self.description(), channel_type),
            ResourceViewError::Layer(ref le) => write!(f, "{}: {}", self.description(), le),
            _ => write!(f, "{}", self.description())
        }
    }
}

impl Error for ResourceViewError {
    fn description(&self) -> &str {
        match *self {
            ResourceViewError::NoBindFlag => "The corresponding bind flag is not present in the texture",
            ResourceViewError::Channel(_) => "Selected channel type is not supported for this texture",
            ResourceViewError::Layer(_) => "Selected layer can not be viewed for this texture",
            ResourceViewError::Unsupported => "The backend was refused for some reason",
        }
    }

    fn cause(&self) -> Option<&Error> {
        if let ResourceViewError::Layer(ref e) = *self {
            Some(e)
        } else {
            None
        }
    }
}

/// Error creating either a RenderTargetView, or DepthStencilView.
#[derive(Clone, Debug, PartialEq)]
pub enum TargetViewError {
    /// The `RENDER_TARGET`/`DEPTH_STENCIL` flag is not present in the texture.
    NoBindFlag,
    /// Selected mip level doesn't exist.
    Level(target::Level),
    /// Selected array layer doesn't exist.
    Layer(image::LayerError),
    /// Selected channel type is not supported for this texture.
    Channel(format::ChannelType),
    /// The backend was refused for some reason.
    Unsupported,
    /// The RTV cannot be changed due to the references to it existing.
    NotDetached
}

impl fmt::Display for TargetViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let description = self.description();
        match *self {
            TargetViewError::Level(ref level) => write!(f, "{}: {}", description, level),
            TargetViewError::Layer(ref layer) => write!(f, "{}: {}", description, layer),
            TargetViewError::Channel(ref channel)  => write!(f, "{}: {:?}", description, channel),
            _ => write!(f, "{}", description)
        }
    }
}

impl Error for TargetViewError {
    fn description(&self) -> &str {
        match *self {
            TargetViewError::NoBindFlag =>
                "The `RENDER_TARGET`/`DEPTH_STENCIL` flag is not present in the texture",
            TargetViewError::Level(_) =>
                "Selected mip level doesn't exist",
            TargetViewError::Layer(_) =>
                "Selected array layer doesn't exist",
            TargetViewError::Channel(_) =>
                "Selected channel type is not supported for this texture",
            TargetViewError::Unsupported =>
                "The backend was refused for some reason",
            TargetViewError::NotDetached =>
                "The RTV cannot be changed due to the references to it existing",
        }
    }

    fn cause(&self) -> Option<&Error> {
        if let TargetViewError::Layer(ref e) = *self {
            Some(e)
        } else {
            None
        }
    }
}

/// An error from creating textures with views at the same time.
#[derive(Clone, Debug, PartialEq)]
pub enum CombinedError {
    /// Failed to create the raw texture.
    Texture(image::CreationError),
    /// Failed to create SRV or UAV.
    Resource(ResourceViewError),
    /// Failed to create RTV or DSV.
    Target(TargetViewError),
}

impl fmt::Display for CombinedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CombinedError::Texture(ref e) => write!(f, "{}: {}", self.description(), e),
            CombinedError::Resource(ref e) => write!(f, "{}: {}", self.description(), e),
            CombinedError::Target(ref e) => write!(f, "{}: {}", self.description(), e),
        }
    }
}

impl Error for CombinedError {
    fn description(&self) -> &str {
        match *self {
            CombinedError::Texture(_) => "Failed to create the raw texture",
            CombinedError::Resource(_) => "Failed to create SRV or UAV",
            CombinedError::Target(_) => "Failed to create RTV or DSV",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            CombinedError::Texture(ref e) => Some(e),
            CombinedError::Resource(ref e) => Some(e),
            CombinedError::Target(ref e) => Some(e),
        }
    }
}

impl From<image::CreationError> for CombinedError {
    fn from(e: image::CreationError) -> CombinedError {
        CombinedError::Texture(e)
    }
}
impl From<ResourceViewError> for CombinedError {
    fn from(e: ResourceViewError) -> CombinedError {
        CombinedError::Resource(e)
    }
}
impl From<TargetViewError> for CombinedError {
    fn from(e: TargetViewError) -> CombinedError {
        CombinedError::Target(e)
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
/// ## Immutable resources
///
/// Immutable buffers and textures can only be read by the GPU. They cannot be written by the GPU and
/// cannot be accessed at all by the CPU.
///
/// See:
///  - [`Device::create_texture_immutable`](trait.Device.html#tymethod.create_texture_immutable),
///  - [`Device::create_buffer_immutable`](trait.Device.html#tymethod.create_buffer_immutable).
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
pub trait Device<B: Backend> {
    /// Returns the features of this `Device`. This usually depends on the graphics API being
    /// used.
    fn get_features(&self) -> &Features;

    /// Returns the limits of this `Device`.
    fn get_limits(&self) -> &Limits;

    /// Create an heap of a specific type.
    ///
    /// There is only a limited amount of allocations allowed depending on the implementation!
    fn create_heap(&mut self, heap_type: &HeapType, resource_type: ResourceHeapType, size: u64) -> Result<B::Heap, ResourceHeapError>;

    ///
    fn create_renderpass(&mut self, attachments: &[pass::Attachment], subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> B::RenderPass;

    ///
    fn create_pipeline_layout(&mut self, sets: &[&B::DescriptorSetLayout]) -> B::PipelineLayout;

    /// Create graphics pipelines.
    fn create_graphics_pipelines<'a>(&mut self, &[(&B::ShaderLib, &B::PipelineLayout, pass::Subpass<'a, B>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<B::GraphicsPipeline, pso::CreationError>>;

    /// Create compute pipelines.
    fn create_compute_pipelines(
        &mut self,
        &[(&B::ShaderLib, pso::EntryPoint, &B::PipelineLayout)],
    ) -> Vec<Result<B::ComputePipeline, pso::CreationError>>;

    ///
    fn create_framebuffer(
        &mut self,
        renderpass: &B::RenderPass,
        color_attachments: &[&B::RenderTargetView],
        depth_stencil_attachments: &[&B::DepthStencilView],
        extent: Extent,
    ) -> B::FrameBuffer;

    ///
    fn create_sampler(&mut self, image::SamplerInfo) -> B::Sampler;

    /// Create a new buffer (unbound).
    ///
    /// The created buffer won't have associated memory until `bind_buffer_memory` is called.
    fn create_buffer(&mut self, size: u64, stride: u64, usage: buffer::Usage) -> Result<B::UnboundBuffer, buffer::CreationError>;

    ///
    fn get_buffer_requirements(&mut self, buffer: &B::UnboundBuffer) -> Requirements;

    /// Bind heap memory to a buffer.
    ///
    /// The unbound buffer will be consumed because the binding is *immutable*.
    /// Be sure to check that there is enough memory available for the buffer.
    /// Use `get_buffer_requirements` to acquire the memory requirements.
    fn bind_buffer_memory(&mut self, heap: &B::Heap, offset: u64, buffer: B::UnboundBuffer) -> Result<B::Buffer, buffer::CreationError>;

    ///
    fn create_image(&mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<B::UnboundImage, image::CreationError>;

    ///
    fn get_image_requirements(&mut self, image: &B::UnboundImage) -> Requirements;

    ///
    fn bind_image_memory(&mut self, heap: &B::Heap, offset: u64, image: B::UnboundImage) -> Result<B::Image, image::CreationError>;

    ///
    fn view_buffer_as_constant(&mut self, buffer: &B::Buffer, range: Range<u64>) -> Result<B::ConstantBufferView, TargetViewError>;

    ///
    fn view_image_as_render_target(&mut self, image: &B::Image, format: format::Format, range: image::SubresourceRange) -> Result<B::RenderTargetView, TargetViewError>;

    ///
    fn view_image_as_shader_resource(&mut self, image: &B::Image, format: format::Format) -> Result<B::ShaderResourceView, TargetViewError>;

    ///
    fn view_image_as_unordered_access(&mut self, image: &B::Image, format: format::Format) -> Result<B::UnorderedAccessView, TargetViewError>;

    /// Create a descriptor pool.
    ///
    /// Descriptor pools allow allocation of descriptor sets.
    /// The pool can't be modified directly, only trough updating descriptor sets.
    fn create_descriptor_pool(&mut self, max_sets: usize, descriptor_ranges: &[pso::DescriptorRangeDesc]) -> B::DescriptorPool;

    /// Create a descriptor set layout.
    fn create_descriptor_set_layout(&mut self, bindings: &[pso::DescriptorSetLayoutBinding]) -> B::DescriptorSetLayout;

    ///
    // TODO: copies
    fn update_descriptor_sets(&mut self, writes: &[pso::DescriptorSetWrite<B>]);

    // TODO: mapping requires further looking into.
    // vulkan requires non-coherent mapping to round the range delimiters
    // Nested mapping is not allowed in vulkan.
    // How to handle it properly for backends? Explicit synchronization?

    /// Map a buffer and obtain a raw pointer for reading.
    fn read_mapping_raw(&mut self, buf: &B::Buffer, range: Range<u64>)
        -> Result<(*const u8, B::Mapping), mapping::Error>;

    /// Map a buffer and obtain a raw pointer for writing.
    fn write_mapping_raw(&mut self, buf: &B::Buffer, range: Range<u64>)
        -> Result<(*mut u8, B::Mapping), mapping::Error>;

    /// Unmap a read/write buffer mapping manually.
    fn unmap_mapping_raw(&mut self, mapping: B::Mapping);

    /// Acquire a mapping Reader
    ///
    /// See `write_mapping` for more information.
    fn read_mapping<'a, T>(&mut self, buf: &'a B::Buffer, range: Range<u64>)
        -> Result<mapping::Reader<'a, B, T>, mapping::Error>
    where
        T: Copy,
    {
        let count = (range.end - range.start) as usize / mem::size_of::<T>();
        self.read_mapping_raw(buf, range)
            .map(|(ptr, mapping)| mapping::Reader {
                slice: unsafe { slice::from_raw_parts(ptr as *const _, count) },
                _mapping: mapping,
            })
    }

    /// Acquire a mapping Writer
    ///
    /// While holding this writer, you hold CPU-side exclusive access.
    /// Any access overlap will result in an error.
    /// Submitting commands involving this buffer to the device
    /// implicitly requires exclusive access. Additionally,
    /// further access will be stalled until execution completion.
    fn write_mapping<'a, T>(&mut self, buf: &'a B::Buffer, range: Range<u64>)
        -> Result<mapping::Writer<'a, B, T>, mapping::Error>
    where
        T: Copy,
    {
        let count = (range.end - range.start) as usize / mem::size_of::<T>();
        self.write_mapping_raw(buf, range)
            .map(|(ptr, mapping)| mapping::Writer {
                slice: unsafe { slice::from_raw_parts_mut(ptr as *mut _, count) },
                _mapping: mapping,
            })
    }

    ///
    fn create_semaphore(&mut self) -> B::Semaphore;

    ///
    fn create_fence(&mut self, signaled: bool) -> B::Fence;

    ///
    fn reset_fences(&mut self, fences: &[&B::Fence]);

    /// Blocks until all or one of the given fences are signaled.
    /// Returns true if fences were signaled before the timeout.
    fn wait_for_fences(&mut self, fences: &[&B::Fence], wait: WaitFor, timeout_ms: u32) -> bool;

    ///
    fn destroy_heap(&mut self, B::Heap);

    ///
    fn destroy_shader_lib(&mut self, B::ShaderLib);

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
    fn destroy_framebuffer(&mut self, B::FrameBuffer);

    /// Destroys a buffer.
    ///
    /// The buffer shouldn't be destroy before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_buffer(&mut self, B::Buffer);

    /// Destroys an image.
    ///
    /// The image shouldn't be destroy before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_image(&mut self, B::Image);

    ///
    fn destroy_render_target_view(&mut self, B::RenderTargetView);

    ///
    fn destroy_depth_stencil_view(&mut self, B::DepthStencilView);

    ///
    fn destroy_constant_buffer_view(&mut self, B::ConstantBufferView);

    ///
    fn destroy_shader_resource_view(&mut self, B::ShaderResourceView);

    ///
    fn destroy_unordered_access_view(&mut self, B::UnorderedAccessView);

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
