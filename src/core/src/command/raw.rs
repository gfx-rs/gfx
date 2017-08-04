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

use {memory, pso, state, target};
use {Backend, Resources};
use buffer::IndexBufferView;

pub trait RawCommandBuffer<B: Backend, R: Resources> {

    ///
    fn finish(&mut self) -> B::SubmitInfo;

    ///
    fn pipeline_barrier<'a>(&mut self, &[memory::MemoryBarrier], &[memory::BufferBarrier], &[memory::ImageBarrier]);

    /// Clear depth-stencil target-
    fn clear_depth_stencil(&mut self, &R::DepthStencilView, Option<target::Depth>, Option<target::Stencil>);

    ///
    fn resolve_image(&mut self);

    /// Bind index buffer view.
    fn bind_index_buffer(&mut self, IndexBufferView<R>);

    /// Bind vertex buffers.
    fn bind_vertex_buffers(&mut self, pso::VertexBufferSet<R>);

    ///
    fn set_viewports(&mut self, &[target::Rect]);
    ///
    fn set_scissors(&mut self, &[target::Rect]);
    ///
    fn set_ref_values(&mut self, state::RefValues);

    /// Bind a graphics pipeline.
    ///
    /// There is only *one* pipeline slot for compute and graphics.
    /// Calling the corresponding `bind_pipeline` functions will override the slot.
    fn bind_graphics_pipeline(&mut self, &R::GraphicsPipeline);
    fn bind_graphics_descriptor_sets(&mut self, layout: &R::PipelineLayout, first_set: usize, sets: &[&R::DescriptorSet]);

    fn bind_compute_pipeline(&mut self, &R::ComputePipeline);
    fn dispatch(&mut self, u32, u32, u32);
    fn dispatch_indirect(&mut self);
}
