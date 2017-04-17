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

use std::ops::Range;
use {buffer, format, image, mapping, memory, pass, pso, shade};
use {HeapType, Resources, SubPass};

/// Error creating either a ShaderResourceView, or UnorderedAccessView.
#[derive(Clone, PartialEq, Debug)]
pub enum ResourceViewError { }

/// Error creating either a RenderTargetView, or DepthStencilView.
#[derive(Clone, PartialEq, Debug)]
pub enum TargetViewError { }

/// Type of the descriptor heap. Defines which descriptors can be placed.
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

pub struct DescriptorPoolDesc {
    pub ty: DescriptorType,
    pub count: usize,
}

pub struct DescriptorSetLayoutBinding {
    pub binding: usize,
    pub ty: DescriptorType,
    pub count: usize,
    pub stage_flags: shade::StageFlags,

    // TODO: immutable samplers?
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
    fn create_heap(&mut self, heap_type: &HeapType, size: u64) -> R::Heap;

    ///
    fn create_renderpass(&mut self, attachments: &[pass::Attachment], subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> R::RenderPass;

    ///
    fn create_pipeline_layout(&mut self) -> R::PipelineLayout;

    /// Create graphics pipelines.
    fn create_graphics_pipelines<'a>(&mut self, &[(&R::ShaderLib, &R::PipelineLayout, SubPass<'a, R>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<R::GraphicsPipeline, pso::CreationError>>;

    /// Create compute pipelines.
    fn create_compute_pipelines(&mut self) -> Vec<Result<R::ComputePipeline, pso::CreationError>>;

    ///
    fn create_framebuffer(&mut self, renderpass: &R::RenderPass,
        color_attachments: &[&R::RenderTargetView], depth_stencil_attachments: &[&R::DepthStencilView],
        width: u32, height: u32, layers: u32
    ) -> R::FrameBuffer;

    /// Create a new buffer (unbound).
    ///
    /// The created buffer won't have associated memory until `bind_buffer_memory` is called.
    fn create_buffer(&mut self, size: u64, usage: buffer::Usage) -> Result<R::UnboundBuffer, buffer::CreationError>;

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
    fn view_image_as_render_target(&mut self, image: &R::Image, format: format::Format) -> Result<R::RenderTargetView, TargetViewError>;

    ///
    fn view_image_as_shader_resource(&mut self) -> Result<R::ShaderResourceView, TargetViewError>;

    ///
    fn create_descriptor_heap(&mut self, ty: DescriptorHeapType, size: usize) -> R::DescriptorHeap;

    ///
    fn create_descriptor_set_pool(&mut self, heap: &R::DescriptorHeap, max_sets: usize, offset: usize, descriptor_pools: &[DescriptorPoolDesc]) -> R::DescriptorSetPool;

    ///
    fn create_descriptor_set_layout(&mut self, bindings: &[DescriptorSetLayoutBinding]) -> R::DescriptorSetLayout;

    ///
    fn create_descriptor_sets(&mut self, set_pool: &mut R::DescriptorSetPool, layout: &[&R::DescriptorSetLayout]) -> Vec<R::DescriptorSet>;

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

    fn destroy_heap(&mut self, R::Heap);

    fn destroy_shader_lib(&mut self, R::ShaderLib);

    fn destroy_renderpass(&mut self, R::RenderPass);

    fn destroy_pipeline_layout(&mut self, R::PipelineLayout);

    fn destroy_graphics_pipeline(&mut self, R::GraphicsPipeline);

    fn destroy_compute_pipeline(&mut self, R::ComputePipeline);

    fn destroy_framebuffer(&mut self, R::FrameBuffer);

    fn destroy_buffer(&mut self, R::Buffer);

    fn destroy_image(&mut self, R::Image);

    fn destroy_render_target_view(&mut self, R::RenderTargetView);
}
