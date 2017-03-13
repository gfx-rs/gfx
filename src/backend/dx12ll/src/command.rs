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
use core::{self, command};
use native::{self, GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer};
use {Resources as R};
use winapi;

pub struct SubmitInfo(pub ComPtr<winapi::ID3D12GraphicsCommandList>);

impl command::CommandBuffer for GeneralCommandBuffer {
    type SubmitInfo = SubmitInfo;
    unsafe fn end(&mut self) -> SubmitInfo {
        unimplemented!()
    }
}

impl command::CommandBuffer for GraphicsCommandBuffer {
    type SubmitInfo = SubmitInfo;
    unsafe fn end(&mut self) -> SubmitInfo {
        unimplemented!()
    }
}

impl core::PrimaryCommandBuffer<R> for GraphicsCommandBuffer {
    fn pipeline_barrier(&mut self) {
        unimplemented!()
    }

    fn execute_commands(&mut self) {
        unimplemented!()
    }
}

impl core::TransferCommandBuffer<R> for GraphicsCommandBuffer {
    fn update_buffer(&mut self, buffer: &(), data: &[u8], offset: usize) {
        unimplemented!()
    }

    fn copy_buffer(&mut self, src: &(), dest: &(), _: &[command::BufferCopy]) {
        unimplemented!()
    }

    fn copy_image(&mut self, src: &(), dest: &()) {
        unimplemented!()
    }

    fn copy_buffer_to_image(&mut self) {
        unimplemented!()
    }

    fn copy_image_to_buffer(&mut self) {
        unimplemented!()
    } 
}

impl core::ProcessingCommandBuffer<R> for GraphicsCommandBuffer {
    fn clear_color(&mut self, rtv: &(), value: command::ClearColor) {
        unimplemented!()
    }

    fn clear_buffer(&mut self) {
        unimplemented!()
    }

    fn bind_pipeline(&mut self, pso: &native::Pipeline) {
        unimplemented!()
    }

    fn bind_descriptor_sets(&mut self) {
        unimplemented!()
    }

    fn push_constants(&mut self) {
        unimplemented!()
    }
}

impl command::CommandBuffer for TransferCommandBuffer {
    type SubmitInfo = SubmitInfo;
    unsafe fn end(&mut self) -> SubmitInfo {
        unimplemented!()
    }
}

impl command::CommandBuffer for ComputeCommandBuffer {
    type SubmitInfo = SubmitInfo;
    unsafe fn end(&mut self) -> SubmitInfo {
        unimplemented!()
    }
}

impl command::CommandBuffer for SubpassCommandBuffer {
    type SubmitInfo = SubmitInfo;
    unsafe fn end(&mut self) -> SubmitInfo {
        unimplemented!()
    }
}
