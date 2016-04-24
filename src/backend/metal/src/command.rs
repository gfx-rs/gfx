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

#![allow(missing_docs)]

use gfx_core::{draw, pso, shade, state, target, tex};
use gfx_core::{IndexType, VertexCount};
use gfx_core::{MAX_VERTEX_ATTRIBUTES, MAX_CONSTANT_BUFFERS,
               MAX_RESOURCE_VIEWS, MAX_UNORDERED_VIEWS,
               MAX_SAMPLERS, MAX_COLOR_TARGETS};

use {Resources, InputLayout, Buffer, Texture, Pipeline, Program};
use native::{Rtv, Srv, Dsv};

use metal::*;

pub struct CommandBuffer {
    cmd_buf: MTLCommandBuffer,
    encoder: MTLRenderCommandEncoder,
}

impl draw::CommandBuffer<Resources> for CommandBuffer {
    fn clone_empty(&self) -> Self {
        unimplemented!()        
    }

    fn reset(&mut self) {
        
    }

    fn bind_pipeline_state(&mut self, pso: Pipeline) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<Resources>) {
        unimplemented!()
    }

    fn bind_constant_buffers(&mut self, cb: &[pso::ConstantBufferParam<Resources>]) {
        unimplemented!()
    }

    fn bind_global_constant(&mut self, gc: shade::Location, value: shade::UniformValue) {
        unimplemented!()
    }

    fn bind_resource_views(&mut self, rvs: &[pso::ResourceViewParam<Resources>]) {
        unimplemented!()
    }

    fn bind_unordered_views(&mut self, uvs: &[pso::UnorderedViewParam<Resources>]) {
        unimplemented!()
    }

    fn bind_samplers(&mut self, samplers: &[pso::SamplerParam<Resources>]) {
        unimplemented!()
    }

    fn bind_pixel_targets(&mut self, targets: pso::PixelTargetSet<Resources>) {
        unimplemented!()
    }

    fn bind_index(&mut self, buf: Buffer, itype: IndexType) {
        unimplemented!()
    }

    fn set_scissor(&mut self, rect: target::Rect) {
        unimplemented!()
    }

    fn set_ref_values(&mut self, vals: state::RefValues) {
        unimplemented!()
    }

    fn update_buffer(&mut self, buf: Buffer, data: &[u8], offset: usize) {
        unimplemented!()
    }

    fn update_texture(&mut self, tex: Texture, kind: tex::Kind, face: Option<tex::CubeFace>,
                      data: &[u8], info: tex::RawImageInfo) {
        unimplemented!()
    }

    fn generate_mipmap(&mut self, srv: Srv) {
        unimplemented!()
    }

    fn clear_color(&mut self, rtv: Rtv, clear: draw::ClearColor) {
        unimplemented!()
    }

    fn clear_depth_stencil(&mut self, dsview: Dsv,
                           depth: Option<target::Depth>, stencil: Option<target::Stencil>) {
        unimplemented!()
    }

    fn call_draw(&mut self, start: VertexCount, count: VertexCount, instances: draw::InstanceOption) {
        unimplemented!()
    }

    fn call_draw_indexed(&mut self, start: VertexCount, count: VertexCount,
                         base: VertexCount, instances: draw::InstanceOption) {
        unimplemented!()
    }
}
