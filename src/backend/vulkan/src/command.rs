// Copyright 2016 The Gfx-rs Developers.
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

use std::mem;
use vk;
use gfx_core::{self as core, draw, pso, shade, target, tex};
use gfx_core::state::RefValues;
use gfx_core::{IndexType, VertexCount};
use {Error, Resources, Share, SharePointer};


pub struct Buffer {
    inner: vk::CommandBuffer,
}

impl draw::CommandBuffer<Resources> for Buffer {
    fn reset(&mut self) {}
    fn bind_pipeline_state(&mut self, _: ()) {}
    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<Resources>) {}
    fn bind_constant_buffers(&mut self, _: &[pso::ConstantBufferParam<Resources>]) {}
    fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {}
    fn bind_resource_views(&mut self, _: &[pso::ResourceViewParam<Resources>]) {}
    fn bind_unordered_views(&mut self, _: &[pso::UnorderedViewParam<Resources>]) {}
    fn bind_samplers(&mut self, _: &[pso::SamplerParam<Resources>]) {}
    fn bind_pixel_targets(&mut self, _: pso::PixelTargetSet<Resources>) {}
    fn bind_index(&mut self, _: (), _: IndexType) {}
    fn set_scissor(&mut self, _: target::Rect) {}
    fn set_ref_values(&mut self, _: RefValues) {}
    fn update_buffer(&mut self, _: (), _: &[u8], _: usize) {}
    fn update_texture(&mut self, _: (), _: tex::Kind, _: Option<tex::CubeFace>,
                      _: &[u8], _: tex::RawImageInfo) {}
    fn generate_mipmap(&mut self, _: ()) {}
    fn clear_color(&mut self, _: (), _: draw::ClearColor) {}
    fn clear_depth_stencil(&mut self, _: (), _: Option<target::Depth>,
                           _: Option<target::Stencil>) {}
    fn call_draw(&mut self, _: VertexCount, _: VertexCount, _: draw::InstanceOption) {}
    fn call_draw_indexed(&mut self, _: VertexCount, _: VertexCount,
                         _: VertexCount, _: draw::InstanceOption) {}
}


pub struct GraphicsQueue {
    share: SharePointer,
    queue: vk::Queue,
    capabilities: core::Capabilities,
}

impl GraphicsQueue {
    #[doc(hidden)]
    pub fn new(share: SharePointer, q: vk::Queue) -> GraphicsQueue {
        let caps = core::Capabilities {
            max_vertex_count: 0,
            max_index_count: 0,
            max_texture_size: 0,
            instance_base_supported: false,
            instance_call_supported: false,
            instance_rate_supported: false,
            vertex_base_supported: false,
            srgb_color_supported: false,
            constant_buffer_supported: false,
            unordered_access_view_supported: false,
            separate_blending_slots_supported: false,
        };
        GraphicsQueue {
            share: share,
            queue: q,
            capabilities: caps,
        }
    }

    pub fn get_share(&self) -> &Share {
        &self.share
    }
}

impl core::Device for GraphicsQueue {
    type Resources = Resources;
    type CommandBuffer = Buffer;

    fn get_capabilities(&self) -> &core::Capabilities {
        &self.capabilities
    }

    fn pin_submitted_resources(&mut self, _: &core::handle::Manager<Resources>) {}

    fn submit(&mut self, com: &mut Buffer) {
        let vk = self.share.dev_pointers();
        let status = unsafe {
            vk.EndCommandBuffer(com.inner)
        };
        if status != vk::SUCCESS {
            panic!("vkEndCommandBuffer: {:?}", Error(status));
        }
        let info = vk::SubmitInfo {
            sType: vk::STRUCTURE_TYPE_SUBMIT_INFO,
            commandBufferCount: 1,
            pCommandBuffers: &com.inner,
            .. unsafe { mem::zeroed() }
        };
        let status = unsafe {
            vk.QueueSubmit(self.queue, 1, &info, 0)
        };
        if status != vk::SUCCESS {
            panic!("vkQueueSubmit: {:?}", Error(status));
        }
    }

    fn cleanup(&mut self) {}
}
