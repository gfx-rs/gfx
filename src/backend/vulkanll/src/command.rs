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

use std::ptr;

use core::{self, command, pso, state, target, IndexType, VertexCount};
use native::{self, CommandBuffer, GeneralCommandBuffer, GraphicsCommandBuffer, ComputeCommandBuffer, TransferCommandBuffer, SubpassCommandBuffer};
use {Resources as R};

pub struct SubmitInfo;


impl CommandBuffer {
    fn end(&mut self) -> SubmitInfo {
        unimplemented!()
    }

    fn pipeline_barrier(&mut self) {
        unimplemented!()
    }

    fn execute_commands(&mut self) {
        unimplemented!()
    }

    fn update_buffer(&mut self, buffer: &native::Buffer, data: &[u8], offset: usize) {
        unimplemented!()
    }

    fn copy_buffer(&mut self, src: &native::Buffer, dst: &native::Buffer, regions: Option<&[command::BufferCopy]>) {
        unimplemented!()
    }

    fn copy_image(&mut self, src: &native::Image, dest: &native::Image) {
        unimplemented!()
    }

    fn copy_buffer_to_image(&mut self) {
        unimplemented!()
    }

    fn copy_image_to_buffer(&mut self) {
        unimplemented!()
    }

    fn clear_color(&mut self, rtv: &(), value: command::ClearColor) {
        unimplemented!()
    }

    fn clear_buffer(&mut self) {
        unimplemented!()
    }

    fn bind_pipeline(&mut self, pso: &()) {
        unimplemented!()
    }

    fn bind_descriptor_sets(&mut self) {
        unimplemented!()
    }

    fn push_constants(&mut self) {
        unimplemented!()
    }

    fn clear_attachment(&mut self) {
        unimplemented!()
    }

    fn draw(&mut self, start: VertexCount, count: VertexCount, instances: Option<command::InstanceParams>) {
        unimplemented!()
    }

    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexCount, instances: Option<command::InstanceParams>) {
        unimplemented!()
    }

    fn draw_indirect(&mut self) {
        unimplemented!()
    }

    fn draw_indexed_indirect(&mut self) {
        unimplemented!()
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        unimplemented!()
    }

    fn dispatch_indirect(&mut self) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ib: &native::Buffer, index_type: IndexType) {
        unimplemented!()
    }

    fn bind_vertex_buffers(&mut self, _: pso::VertexBufferSet<R>) {
        unimplemented!()
    }

    fn set_viewports(&mut self, viewports: &[target::Rect]) {
        unimplemented!()
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        unimplemented!()
    }

    fn set_ref_values(&mut self, _: state::RefValues) {
        unimplemented!()
    }

    fn clear_depth_stencil(&mut self, _: &(), depth: Option<target::Depth>, stencil: Option<target::Stencil>) {
        unimplemented!()
    }

    fn resolve_image(&mut self) {
        unimplemented!()
    }
}

// CommandBuffer trait implementation
macro_rules! impl_cmd_buffer {
    ($buffer:ident) => (
        impl command::CommandBuffer for $buffer {
            type SubmitInfo = SubmitInfo;
            unsafe fn end(&mut self) -> SubmitInfo {
                self.0.end()
            }
        }
    )
}

impl_cmd_buffer!(GeneralCommandBuffer);
impl_cmd_buffer!(GraphicsCommandBuffer);
impl_cmd_buffer!(ComputeCommandBuffer);
impl_cmd_buffer!(TransferCommandBuffer);
impl_cmd_buffer!(SubpassCommandBuffer);

// PrimaryCommandBuffer trait implementation
macro_rules! impl_primary_cmd_buffer {
    ($buffer:ident) => (
        impl core::PrimaryCommandBuffer<R> for $buffer {
            fn pipeline_barrier(&mut self) {
                self.0.pipeline_barrier()
            }

            fn execute_commands(&mut self) {
                self.0.execute_commands()
            }
        }
    )
}

impl_primary_cmd_buffer!(GeneralCommandBuffer);
impl_primary_cmd_buffer!(GraphicsCommandBuffer);
impl_primary_cmd_buffer!(ComputeCommandBuffer);
impl_primary_cmd_buffer!(TransferCommandBuffer);

// ProcessingCommandBuffer trait implementation
macro_rules! impl_processing_cmd_buffer {
    ($buffer:ident) => (
        impl core::ProcessingCommandBuffer<R> for $buffer {
            fn clear_color(&mut self, rtv: &(), value: command::ClearColor) {
                self.0.clear_color(rtv, value)
            }

            fn clear_buffer(&mut self) {
                self.0.clear_buffer()
            }

            fn bind_pipeline(&mut self, pso: &()) {
                self.0.bind_pipeline(pso)
            }

            fn bind_descriptor_sets(&mut self) {
                self.0.bind_descriptor_sets()
            }

            fn push_constants(&mut self) {
                self.0.push_constants()
            }
        }
    )
}

impl_processing_cmd_buffer!(GeneralCommandBuffer);
impl_processing_cmd_buffer!(GraphicsCommandBuffer);
impl_processing_cmd_buffer!(ComputeCommandBuffer);

// TransferCommandBuffer trait implementation
macro_rules! impl_transfer_cmd_buffer {
    ($buffer:ident) => (
        impl core::TransferCommandBuffer<R> for $buffer {
            fn update_buffer(&mut self, buffer: &native::Buffer, data: &[u8], offset: usize) {
                self.0.update_buffer(buffer, data, offset)
            }

            fn copy_buffer(&mut self, src: &native::Buffer, dest: &native::Buffer, regions: Option<&[command::BufferCopy]>) {
                self.0.copy_buffer(src, dest, regions)
            }

            fn copy_image(&mut self, src: &native::Image, dest: &native::Image) {
                self.0.copy_image(src, dest)
            }

            fn copy_buffer_to_image(&mut self) {
                self.0.copy_buffer_to_image()
            }

            fn copy_image_to_buffer(&mut self) {
                self.0.copy_image_to_buffer()
            } 
        }
    )
}

impl_transfer_cmd_buffer!(GeneralCommandBuffer);
impl_transfer_cmd_buffer!(GraphicsCommandBuffer);
impl_transfer_cmd_buffer!(ComputeCommandBuffer);
impl_transfer_cmd_buffer!(TransferCommandBuffer);

// GraphicsCommandBuffer trait implementation
macro_rules! impl_graphics_cmd_buffer {
    ($buffer:ident) => (
        impl core::GraphicsCommandBuffer<R> for $buffer {
            fn clear_depth_stencil(&mut self, dsv: &(), depth: Option<target::Depth>, stencil: Option<target::Stencil>) {
                self.0.clear_depth_stencil(dsv, depth, stencil)
            }

            fn resolve_image(&mut self) {
                self.0.resolve_image()
            }

            fn bind_index_buffer(&mut self, buffer: &native::Buffer, index_type: IndexType) {
                self.0.bind_index_buffer(buffer, index_type)
            }

            fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<R>) {
                self.0.bind_vertex_buffers(vbs)
            }

            fn set_viewports(&mut self, viewports: &[target::Rect]) {
                self.0.set_viewports(viewports)
            }

            fn set_scissors(&mut self, scissors: &[target::Rect]) {
                self.0.set_scissors(scissors)
            }

            fn set_ref_values(&mut self, rv: state::RefValues) {
                self.0.set_ref_values(rv)
            }
        }
    )
}

impl_graphics_cmd_buffer!(GeneralCommandBuffer);
impl_graphics_cmd_buffer!(GraphicsCommandBuffer);

// ComputeCommandBuffer trait implementation
macro_rules! impl_graphics_cmd_buffer {
    ($buffer:ident) => (
        impl core::ComputeCommandBuffer<R> for $buffer {
            fn dispatch(&mut self, x: u32, y: u32, z: u32) {
                self.0.dispatch(x, y, z)
            }

            fn dispatch_indirect(&mut self) {
                self.0.dispatch_indirect()
            }
        }
    )
}

impl_graphics_cmd_buffer!(GeneralCommandBuffer);
impl_graphics_cmd_buffer!(ComputeCommandBuffer);

// TODO: subpass command buffer
