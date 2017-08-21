// Copyright 2017 The Gfx-rs Developers.
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

use {buffer, format, image, mapping, memory, pass, pso, shade};
use {HeapType, Resources, SubPass};
use std::error::Error;
use std::fmt;

/// Error creating either a ShaderResourceView, or UnorderedAccessView.
#[derive(Clone, PartialEq, Debug)]
pub enum ResourceViewError { }

impl fmt::Display for ResourceViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {}
    }
}

impl Error for ResourceViewError {
    fn description(&self) -> &str {
        match *self {}
    }
}

/// Error creating either a RenderTargetView, or DepthStencilView.
#[derive(Clone, PartialEq, Debug)]
pub enum TargetViewError {
    BadFormat,
}

impl fmt::Display for TargetViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(self.description())
    }
}

impl Error for TargetViewError {
    fn description(&self) -> &str {
        match *self {
            TargetViewError::BadFormat => "an incompatible format was requested for the target view",
        }
    }
}

/// Type of the resources that can be allocated on a heap.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResourceHeapType {
    Any,
    Buffers,
    Images,
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

impl fmt::Display for ResourceHeapError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(self.description())
    }
}

impl Error for ResourceHeapError {
    fn description(&self) -> &str {
        match *self {
            ResourceHeapError::UnsupportedType => "the requested type is not supported",
            ResourceHeapError::OutOfMemory => "unable to allocate the specified size",
        }
    }
}

/// Type of the descriptor heap.
///
/// Defines which descriptors can be placed in a descriptor heap.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DescriptorHeapType {
    /// Supports shader resource views (SRV), constant buffer views (CBV) and unordered access views (UAV).
    SrvCbvUav,
    /// Supports samplers only.
    Sampler,
}

// TODO: reevaluate the names
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DescriptorType {
    Sampler,
    SampledImage,
    StorageImage,
    UniformTexelBuffer,
    StorageTexelBuffer,
    ConstantBuffer,
    StorageBuffer,
    InputAttachment,

    // TODO: CombinedImageSampler,
    // ConstantBufferDynamic, StorageBufferDynamic
}

/// Pool of descriptors of a specific type.
#[derive(Clone, Copy, Debug)]
pub struct DescriptorPoolDesc {
    /// Type of the stored descriptors.
    pub ty: DescriptorType,
    /// Amount of space.
    pub count: usize,
}

/// Binding descriptiong of a descriptor set
///
/// A descriptor set consists of multiple binding points.
/// Each binding point contains one or multiple descriptors of a certain type.
/// The binding point is only valid for the pipelines stages specified.
///
/// The binding _must_ match with the corresponding shader interface.
#[derive(Clone, Copy, Debug)]
pub struct DescriptorSetLayoutBinding {
    /// Integer identifier of the binding.
    pub binding: usize,
    /// Type of the bound descriptors.
    pub ty: DescriptorType,
    /// Number of descriptors bound.
    pub count: usize,
    /// Valid shader stages.
    pub stage_flags: shade::StageFlags,

    // TODO: immutable samplers?
}

pub struct DescriptorSetWrite<'a, 'b, R: Resources> {
    pub set: &'a R::DescriptorSet,
    pub binding: usize,
    pub array_offset: usize,
    pub write: DescriptorWrite<'b, R>,
}

// TODO
pub enum DescriptorWrite<'a, R: Resources> {
    Sampler(Vec<&'a R::Sampler>),
    SampledImage(Vec<(&'a R::ShaderResourceView, memory::ImageLayout)>),
    StorageImage(Vec<(&'a R::ShaderResourceView, memory::ImageLayout)>),
    UniformTexelBuffer,
    StorageTexelBuffer,
    ConstantBuffer(Vec<&'a R::ConstantBufferView>),
    StorageBuffer,
    InputAttachment(Vec<(&'a R::ShaderResourceView, memory::ImageLayout)>),
}

/// Specifies the waiting targets.
#[derive(Clone, Copy, Debug)]
pub enum WaitFor {
    /// Wait for any target.
    Any,
    /// Wait for all targets at once.
    All,
}

/// A `Factory` is responsible for creating and managing resources for the backend it was created
/// with.
///
/// This factory structure can then be used to create and manage different resources, like buffers,
/// pipelines and textures. See the individual methods for more information.
#[allow(missing_docs)]
pub trait Factory<R: Resources> {
    /// Create an heap of a specific type.
    ///
    /// There is only a limited amount of allocations allowed depending on the implementation!
    fn create_heap(&mut self, heap_type: &HeapType, resource_type: ResourceHeapType, size: u64) -> Result<R::Heap, ResourceHeapError>;

    ///
    fn create_renderpass(&mut self, attachments: &[pass::Attachment], subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> R::RenderPass;

    ///
    fn create_pipeline_layout(&mut self, sets: &[&R::DescriptorSetLayout]) -> R::PipelineLayout;

    /// Create graphics pipelines.
    fn create_graphics_pipelines<'a>(&mut self, &[(&R::ShaderLib, &R::PipelineLayout, SubPass<'a, R>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<R::GraphicsPipeline, pso::CreationError>>;

    /// Create compute pipelines.
    fn create_compute_pipelines(&mut self, &[(&R::ShaderLib, pso::EntryPoint, &R::PipelineLayout)]) -> Vec<Result<R::ComputePipeline, pso::CreationError>>;

    ///
    fn create_framebuffer(&mut self, renderpass: &R::RenderPass,
        color_attachments: &[&R::RenderTargetView], depth_stencil_attachments: &[&R::DepthStencilView],
        width: u32, height: u32, layers: u32
    ) -> R::FrameBuffer;

    ///
    fn create_sampler(&mut self, image::SamplerInfo) -> R::Sampler;

    /// Create a new buffer (unbound).
    ///
    /// The created buffer won't have associated memory until `bind_buffer_memory` is called.
    fn create_buffer(&mut self, size: u64, stride: u64, usage: buffer::Usage) -> Result<R::UnboundBuffer, buffer::CreationError>;

    ///
    fn get_buffer_requirements(&mut self, buffer: &R::UnboundBuffer) -> memory::MemoryRequirements;

    /// Bind heap memory to a buffer.
    ///
    /// The unbound buffer will be consumed because the binding is *immutable*.
    /// Be sure to check that there is enough memory available for the buffer.
    /// Use `get_buffer_requirements` to acquire the memory requirements.
    fn bind_buffer_memory(&mut self, heap: &R::Heap, offset: u64, buffer: R::UnboundBuffer) -> Result<R::Buffer, buffer::CreationError>;

    ///
    fn create_image(&mut self, kind: image::Kind, mip_levels: image::Level, format: format::Format, usage: image::Usage)
         -> Result<R::UnboundImage, image::CreationError>;

    ///
    fn get_image_requirements(&mut self, image: &R::UnboundImage) -> memory::MemoryRequirements;

    ///
    fn bind_image_memory(&mut self, heap: &R::Heap, offset: u64, image: R::UnboundImage) -> Result<R::Image, image::CreationError>;

    ///
    fn view_buffer_as_constant(&mut self, buffer: &R::Buffer, offset: usize, size: usize) -> Result<R::ConstantBufferView, TargetViewError>;

    ///
    fn view_image_as_render_target(&mut self, image: &R::Image, format: format::Format) -> Result<R::RenderTargetView, TargetViewError>;

    ///
    fn view_image_as_shader_resource(&mut self, image: &R::Image, format: format::Format) -> Result<R::ShaderResourceView, TargetViewError>;

    ///
    fn view_image_as_unordered_access(&mut self, image: &R::Image, format: format::Format) -> Result<R::UnorderedAccessView, TargetViewError>;

    /// Create a descriptor heap.
    ///
    /// A descriptor heap can store a number of GPU descriptors of a certain group.
    fn create_descriptor_heap(&mut self, ty: DescriptorHeapType, size: usize) -> R::DescriptorHeap;

    /// Create a descriptor set pool inside an heap.
    ///
    /// Descriptor set pools allow allocation of descriptor sets by allocating space inside the heap.
    /// The heap can't be modified directly, only trough updating descriptor sets.
    ///
    /// Pools reserve a contiguous range in the heap. The application _must_ keep track of the used ranges.
    /// Using overlapping ranges at the same time results in undefined behavior, depending on the backend implementation.
    fn create_descriptor_set_pool(&mut self, heap: &R::DescriptorHeap, max_sets: usize, offset: usize, descriptor_pools: &[DescriptorPoolDesc]) -> R::DescriptorSetPool;

    /// Create a descriptor set layout.
    fn create_descriptor_set_layout(&mut self, bindings: &[DescriptorSetLayoutBinding]) -> R::DescriptorSetLayout;

    /// Create one or multiple descriptor sets from a pool.
    ///
    /// Each descriptor set will be allocated from the pool according to the corresponding set layout.
    ///
    /// The descriptor pool _must_ have enough space in to allocate the required descriptors.
    fn create_descriptor_sets(&mut self, set_pool: &mut R::DescriptorSetPool, layout: &[&R::DescriptorSetLayout]) -> Vec<R::DescriptorSet>;

    ///
    fn reset_descriptor_set_pool(&mut self, &mut R::DescriptorSetPool);

    ///
    // TODO: copies
    fn update_descriptor_sets(&mut self, writes: &[DescriptorSetWrite<R>]);

    // TODO: mapping requires further looking into.
    // vulkan requires non-coherent mapping to round the range delimiters
    // Nested mapping is not allowed in vulkan.
    // How to handle it properly for backends? Explicit synchronization?

    /// Acquire a mapping Reader.
    fn read_mapping<'a, T>(&self, buf: &'a R::Buffer, offset: u64, size: u64)
                               -> Result<mapping::Reader<'a, R, T>,
                                         mapping::Error>
        where T: Copy;

    /// Acquire a mapping Writer
    fn write_mapping<'a, 'b, T>(&mut self, buf: &'a R::Buffer, offset: u64, size: u64)
                                -> Result<mapping::Writer<'a, R, T>,
                                          mapping::Error>
        where T: Copy;

    ///
    fn create_semaphore(&mut self) -> R::Semaphore;

    ///
    fn create_fence(&mut self, signaled: bool) -> R::Fence;

    ///
    fn reset_fences(&mut self, fences: &[&R::Fence]);

    /// Blocks until all or one of the given fences are signalled.
    /// Returns true if fences were signalled before the timeout.
    fn wait_for_fences(&mut self, fences: &[&R::Fence], wait: WaitFor, timeout_ms: u32) -> bool;

    ///
    fn destroy_heap(&mut self, R::Heap);

    ///
    fn destroy_shader_lib(&mut self, R::ShaderLib);

    ///
    fn destroy_renderpass(&mut self, R::RenderPass);

    ///
    fn destroy_pipeline_layout(&mut self, R::PipelineLayout);

    /// Destroys a graphics pipeline.
    ///
    /// The graphics pipeline shouldn't be destroy before any submitted command buffer,
    /// which references the graphics pipeline, has finished execution.
    fn destroy_graphics_pipeline(&mut self, R::GraphicsPipeline);

    /// Destroys a compute pipeline.
    ///
    /// The compute pipeline shouldn't be destroy before any submitted command buffer,
    /// which references the compute pipeline, has finished execution.
    fn destroy_compute_pipeline(&mut self, R::ComputePipeline);

    /// Destroys a framebuffer.
    ///
    /// The framebuffer shouldn't be destroy before any submitted command buffer,
    /// which references the framebuffer, has finished execution.
    fn destroy_framebuffer(&mut self, R::FrameBuffer);

    /// Destroys a buffer.
    ///
    /// The buffer shouldn't be destroy before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_buffer(&mut self, R::Buffer);

    /// Destroys an image.
    ///
    /// The image shouldn't be destroy before any submitted command buffer,
    /// which references the images, has finished execution.
    fn destroy_image(&mut self, R::Image);

    ///
    fn destroy_render_target_view(&mut self, R::RenderTargetView);

    ///
    fn destroy_depth_stencil_view(&mut self, R::DepthStencilView);

    ///
    fn destroy_constant_buffer_view(&mut self, R::ConstantBufferView);

    ///
    fn destroy_shader_resource_view(&mut self, R::ShaderResourceView);

    ///
    fn destroy_unordered_access_view(&mut self, R::UnorderedAccessView);

    ///
    fn destroy_sampler(&mut self, R::Sampler);

    ///
    fn destroy_descriptor_heap(&mut self, R::DescriptorHeap);

    ///
    fn destroy_descriptor_set_pool(&mut self, R::DescriptorSetPool);

    ///
    fn destroy_descriptor_set_layout(&mut self, R::DescriptorSetLayout);

    ///
    fn destroy_fence(&mut self, R::Fence);

    ///
    fn destroy_semaphore(&mut self, R::Semaphore);
}
