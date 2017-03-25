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
    /// 
    fn create_heap(&mut self, heap_type: &HeapType, size: u64) -> R::Heap;

    ///
    fn create_renderpass(&mut self, attachments: &[pass::Attachment], subpasses: &[pass::SubpassDesc], dependencies: &[pass::SubpassDependency]) -> R::RenderPass;

    ///
    fn create_pipeline_layout(&mut self) -> R::PipelineLayout;

    ///
    fn create_graphics_pipelines<'a>(&mut self, &[(&R::ShaderLib, &R::PipelineLayout, SubPass<'a, R>, &pso::GraphicsPipelineDesc)])
            -> Vec<Result<R::GraphicsPipeline, pso::CreationError>>;

    ///
    fn create_compute_pipelines(&mut self) -> Vec<Result<R::ComputePipeline, pso::CreationError>>;

    ///
    fn create_framebuffer(&mut self, renderpass: &R::RenderPass,
        color_attachments: &[&R::RenderTargetView], depth_stencil_attachments: &[&R::DepthStencilView],
        width: u32, height: u32, layers: u32
    ) -> R::FrameBuffer;

    ///

    // d3d12
    // HRESULT CreatePlacedResource(
    //  [in]                  ID3D12Heap            *pHeap,
    //                        UINT64                HeapOffset,
    //  [in]            const D3D12_RESOURCE_DESC   *pDesc,
    //                        D3D12_RESOURCE_STATES InitialState,
    //  [in, optional]  const D3D12_CLEAR_VALUE     *pOptimizedClearValue,
    //                        REFIID                riid,
    //  [out, optional]       void                  **ppvResource
    //);

    ///
    fn create_buffer(&mut self, size: u64, usage: buffer::Usage) -> Result<R::UnboundBuffer, buffer::CreationError>;

    ///
    fn get_buffer_requirements(&mut self, buffer: &R::UnboundBuffer) -> memory::MemoryRequirements;

    ///
    fn bind_buffer_memory(&mut self, heap: &R::Heap, offset: u64, buffer: R::UnboundBuffer) -> Result<R::Buffer, buffer::CreationError>;

    ///
    fn create_image(&mut self, heap: &R::Heap, offset: u64) -> Result<R::Image, image::CreationError>;

    ///
    fn view_image_as_render_target(&mut self, image: &R::Image, format: format::Format) -> Result<R::RenderTargetView, TargetViewError>;
}
