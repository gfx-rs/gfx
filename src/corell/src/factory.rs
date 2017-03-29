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

use {buffer, format, image, memory, pass, pso, shade};
use {HeapType, Resources, SubPass};

/// Error creating either a ShaderResourceView, or UnorderedAccessView.
#[derive(Clone, PartialEq, Debug)]
pub enum ResourceViewError { }

/// Error creating either a RenderTargetView, or DepthStencilView.
#[derive(Clone, PartialEq, Debug)]
pub enum TargetViewError { }

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
    fn create_image(&mut self, heap: &R::Heap, offset: u64) -> Result<R::Image, image::CreationError>;

    ///
    fn view_image_as_render_target(&mut self, image: &R::Image, format: format::Format) -> Result<R::RenderTargetView, TargetViewError>;
}
