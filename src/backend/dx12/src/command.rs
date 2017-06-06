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

use comptr::ComPtr;
use core::{command, pso, shade, state, target, texture as tex};
use core::{IndexType, VertexCount};
use native::{GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer};
use winapi;
use {Backend, Resources};

pub struct CommandBuffer {
    pub raw: ComPtr<winapi::ID3D12GraphicsCommandList>,
}

pub struct SubmitInfo; // TODO

impl CommandBuffer {
    fn end(&mut self) -> SubmitInfo {
        unimplemented!()
    }
}

// CommandBuffer trait implementation
macro_rules! impl_cmd_buffer {
    ($buffer:ident) => (
        impl command::CommandBuffer<Backend> for $buffer {
            unsafe fn end(&mut self) -> SubmitInfo {
                self.0.end()
            }
        }

        // temp, can be removed later
        impl command::Buffer<Resources> for $buffer {
            fn reset(&mut self) {
                unimplemented!()
            }

            fn bind_pipeline_state(&mut self, _: ()) {
                unimplemented!()
            }

            fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<Resources>) {
                unimplemented!()
            }

            fn bind_constant_buffers(&mut self, _: &[pso::ConstantBufferParam<Resources>]) {
                unimplemented!()
            }

            fn bind_global_constant(&mut self, _: shade::Location, _: shade::UniformValue) {
                unimplemented!()
            }

            fn bind_resource_views(&mut self, _: &[pso::ResourceViewParam<Resources>]) {
                unimplemented!()
            }

            fn bind_unordered_views(&mut self, _: &[pso::UnorderedViewParam<Resources>]) {
                unimplemented!()
            }

            fn bind_samplers(&mut self, _: &[pso::SamplerParam<Resources>]) {
                unimplemented!()
            }

            fn bind_pixel_targets(&mut self, _: pso::PixelTargetSet<Resources>) {
                unimplemented!()
            }

            fn bind_index(&mut self, _: (), _: IndexType) {
                unimplemented!()
            }

            fn set_scissor(&mut self, _: target::Rect) {
                unimplemented!()
            }

            fn set_ref_values(&mut self, _: state::RefValues) {
                unimplemented!()
            }

            fn copy_buffer(&mut self, src: (), dst: (),
                           src_offset_bytes: usize, dst_offset_bytes: usize,
                           size_bytes: usize) {
                unimplemented!()
            }

            fn copy_buffer_to_texture(&mut self, src: (), src_offset_bytes: usize,
                                      dst: (),
                                      kind: tex::Kind,
                                      face: Option<tex::CubeFace>,
                                      img: tex::RawImageInfo) {
                unimplemented!()
            }

            fn copy_texture_to_buffer(&mut self,
                                      src: (),
                                      kind: tex::Kind,
                                      face: Option<tex::CubeFace>,
                                      img: tex::RawImageInfo,
                                      dst: (), dst_offset_bytes: usize) {
                unimplemented!()
            }

            fn update_buffer(&mut self, buf: (), data: &[u8], offset: usize) {
                unimplemented!()
            }

            fn update_texture(&mut self, tex: (), kind: tex::Kind, face: Option<tex::CubeFace>,
                              data: &[u8], image: tex::RawImageInfo) {
                unimplemented!()
            }

            fn generate_mipmap(&mut self, srv: ()) {
                unimplemented!()
            }

            fn clear_color(&mut self, target: (), value: command::ClearColor) {
                unimplemented!()
            }

            fn clear_depth_stencil(&mut self, target: (), depth: Option<target::Depth>,
                                   stencil: Option<target::Stencil>) {
                unimplemented!()
            }

            fn call_draw(&mut self, start: VertexCount, count: VertexCount, instances: Option<command::InstanceParams>) {
                unimplemented!();
            }

            fn call_draw_indexed(&mut self, start: VertexCount, count: VertexCount,
                                 base: VertexCount, instances: Option<command::InstanceParams>) {
                unimplemented!()
            }
        }
    )
}

impl_cmd_buffer!(GeneralCommandBuffer);
impl_cmd_buffer!(GraphicsCommandBuffer);
impl_cmd_buffer!(ComputeCommandBuffer);
impl_cmd_buffer!(TransferCommandBuffer);
impl_cmd_buffer!(SubpassCommandBuffer);

