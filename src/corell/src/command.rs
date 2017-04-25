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

//! Command Buffer device interface

use std::ops::{Deref, DerefMut};
use {image, memory, state, pso, target};
use buffer::IndexBufferView;
use {InstanceCount, VertexCount, VertexOffset, Resources};

/// A universal clear color supporting integet formats
/// as well as the standard floating-point.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub enum ClearColor {
    /// Standard floating-point vec4 color
    Float([f32; 4]),
    /// Integer vector to clear ivec4 targets.
    Int([i32; 4]),
    /// Unsigned int vector to clear uvec4 targets.
    Uint([u32; 4]),
}

/// Depth-stencil target clear values.
pub struct ClearDepthStencil {
    pub depth: f32,
    pub stencil: u32,
}

/// General clear values for attachments (color or depth-stencil).
pub enum ClearValue {
    Color(ClearColor),
    DepthStencil(ClearDepthStencil),
}

pub struct Offset {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

pub struct Extent {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

/// Region of two buffers for copying.
pub struct BufferCopy {
    /// Buffer region source offset.
    pub src: u64,
    /// Buffer region destionation offset.
    pub dst: u64,
    /// Region size.
    pub size: u64,
}

pub struct BufferImageCopy {
    pub buffer_offset: u64,
    pub image_mip_level: image::Level,
    pub image_base_layer: image::Layer,
    pub image_layers: image::Layer,
    pub image_offset: Offset,
    pub image_extent: Extent,
}

/// Optional instance parameters: (instance count, buffer offset)
pub type InstanceParams = (InstanceCount, VertexCount);

/// Encoder wrapper for a command buffer, providing a safe interface.
///
/// After finishing recording the encoder will be consumed and returns a thread-free `Submit` handle.
/// This handle can be sent to a command queue for execution.
pub struct Encoder<'a, C: CommandBuffer + 'a>(&'a mut C);

impl<'a, C: CommandBuffer> Encoder<'a, C> {
    #[doc(hidden)]
    pub unsafe fn new(buffer: &'a mut C) -> Self {
        Encoder(buffer)
    }

    /// Finish recording commands to the command buffers.
    pub fn finish(self) -> Submit<C> {
        Submit(unsafe { self.0.end() })
    }
}

impl<'a, C> Deref for Encoder<'a, C>
    where C: CommandBuffer
{
    type Target = C;

    fn deref(&self) -> &C {
        self.0
    }
}

impl<'a, C> DerefMut for Encoder<'a, C>
    where C: CommandBuffer
{
    fn deref_mut(&mut self) -> &mut C {
        self.0
    }
}

pub struct Submit<C: CommandBuffer>(C::SubmitInfo);
impl<C: CommandBuffer> Submit<C> {
    #[doc(hidden)]
    pub unsafe fn get_info(&self) -> &C::SubmitInfo {
        &self.0
    }
}

pub trait GraphicsCommandBuffer<R: Resources> : PrimaryCommandBuffer<R> {
    /// Clear depth-stencil target-
    fn clear_depth_stencil(&mut self, &R::DepthStencilView, Option<target::Depth>, Option<target::Stencil>);

    // TODO: investigate how `blit_image` can be emulated on d3d12 e.g compute shader. (useful for mipmap generation)
    fn resolve_image(&mut self);

    /// Bind index buffer view.
    fn bind_index_buffer(&mut self, IndexBufferView<R>);

    /// Bind vertex buffers.
    fn bind_vertex_buffers(&mut self, pso::VertexBufferSet<R>);

    fn set_viewports(&mut self, &[target::Rect]);
    fn set_scissors(&mut self, &[target::Rect]);
    fn set_ref_values(&mut self, state::RefValues);

    /// Bind a graphics pipeline.
    ///
    /// There is only *one* pipeline slot for compute and graphics.
    /// Calling the corresponding `bind_pipeline` functions will override the slot.
    fn bind_graphics_pipeline(&mut self, &R::GraphicsPipeline);

    fn bind_graphics_descriptor_sets(&mut self, layout: &R::PipelineLayout, first_set: usize, sets: &[&R::DescriptorSet]);
}

pub trait RenderPassEncoder<'cb, 'rp, 'fb, 'enc: 'cb, C, R>
    where C: GraphicsCommandBuffer<R> + 'enc,
          R: Resources
{
    type SecondaryEncoder: RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc, C, R>;
    type InlineEncoder: RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc, C, R>;

    fn begin(command_buffer: &'cb mut Encoder<'enc, C>,
             render_pass: &'rp R::RenderPass,
             framebuffer: &'fb R::FrameBuffer,
             render_area: target::Rect,
             clear_values: &[ClearValue]) -> Self;

    /// Move to the next subpass of the current renderpass.
    /// The next subpass will be encoded in secondary command buffers.
    fn next_subpass(self) -> Self::SecondaryEncoder;

    /// Move to the next subpass of the current renderpass.
    /// The next subpass will be encoded inline in the primary buffer.
    fn next_subpass_inline(self) -> Self::InlineEncoder;
}

pub trait RenderPassSecondaryEncoder<'cb, 'rp, 'fb, 'enc: 'cb, C, R> : RenderPassEncoder<'cb, 'rp, 'fb, 'enc, C, R>
    where C: GraphicsCommandBuffer<R> + 'enc,
          R: Resources
{
    // TODO: exectue supass command buffer
}

pub trait RenderPassInlineEncoder<'cb, 'rp, 'fb, 'enc: 'cb, C, R> : RenderPassEncoder<'cb, 'rp, 'fb, 'enc, C, R>
    where C: GraphicsCommandBuffer<R> + 'enc,
          R: Resources
{
    fn clear_attachment(&mut self);

    /// Issue a draw command.
    fn draw(&mut self, start: VertexCount, count: VertexCount, Option<InstanceParams>);
    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexOffset, Option<InstanceParams>);
    fn draw_indirect(&mut self);
    fn draw_indexed_indirect(&mut self);

    fn bind_index_buffer<'a>(&mut self, IndexBufferView<R>);
    fn bind_vertex_buffers(&mut self, pso::VertexBufferSet<R>);

    fn set_viewports(&mut self, &[target::Rect]);
    fn set_scissors(&mut self, &[target::Rect]);
    fn set_ref_values(&mut self, state::RefValues);

    fn bind_graphics_pipeline(&mut self, &R::GraphicsPipeline);
    fn bind_graphics_descriptor_sets(&mut self, layout: &R::PipelineLayout, first_set: usize, sets: &[&R::DescriptorSet]);
    fn push_constants(&mut self);
}

pub trait SubpassCommandBuffer<R: Resources> : SecondaryCommandBuffer<R> {
    fn clear_attachment(&mut self);
    fn draw(&mut self, start: VertexCount, count: VertexCount, Option<InstanceParams>);
    fn draw_indexed(&mut self, start: VertexCount, count: VertexCount, base: VertexOffset, Option<InstanceParams>);
    fn draw_indirect(&mut self);
    fn draw_indexed_indirect(&mut self);

    fn bind_index_buffer(&mut self, IndexBufferView<R>);
    fn bind_vertex_buffers(&mut self, pso::VertexBufferSet<R>);

    fn set_viewports(&mut self, &[target::Rect]);
    fn set_scissors(&mut self, &[target::Rect]);
    fn set_ref_values(&mut self, state::RefValues);

    fn bind_graphics_pipeline(&mut self, &R::GraphicsPipeline);
    fn bind_graphics_descriptor_sets(&mut self, layout: &R::PipelineLayout, first_set: usize, sets: &[&R::DescriptorSet]);
    fn push_constants(&mut self);
}

pub trait ComputeCommandBuffer<R: Resources> : ProcessingCommandBuffer<R> {
    fn bind_compute_pipeline(&mut self, &R::ComputePipeline);
    fn dispatch(&mut self, u32, u32, u32);
    fn dispatch_indirect(&mut self);
}

pub trait ProcessingCommandBuffer<R: Resources> : TransferCommandBuffer<R> {
    // TODO: consider to clear multiple RTVs as vulkan allows multiple subresource ranges
    fn clear_color(&mut self, &R::RenderTargetView, ClearColor);
    fn clear_buffer(&mut self);

    fn bind_descriptor_heaps(&mut self, srv_cbv_uav: Option<&R::DescriptorHeap>, samplers: Option<&R::DescriptorHeap>);
    fn push_constants(&mut self);
}

pub trait TransferCommandBuffer<R: Resources> : PrimaryCommandBuffer<R> {
    fn update_buffer(&mut self, &R::Buffer, data: &[u8], offset: usize);

    // TODO: memory aliasing or overlapping regions will result in undefined behavior!
    fn copy_buffer(&mut self, src: &R::Buffer, dest: &R::Buffer, regions: &[BufferCopy]);
    fn copy_image(&mut self, src: &R::Image, dest: &R::Image);
    fn copy_buffer_to_image(&mut self, src: &R::Buffer, dst: &R::Image, layout: memory::ImageLayout, regions: &[BufferImageCopy]);
    fn copy_image_to_buffer(&mut self); 
}

pub trait PrimaryCommandBuffer<R: Resources>: CommandBuffer {
    fn pipeline_barrier<'a>(&mut self, &[memory::MemoryBarrier], &[memory::BufferBarrier<'a, R>], &[memory::ImageBarrier<'a, R>]);
    fn execute_commands(&mut self);
}

pub trait SecondaryCommandBuffer<R: Resources>: CommandBuffer {
    fn pipeline_barrier(&mut self);
}

pub trait CommandBuffer {
    type SubmitInfo;

    #[doc(hidden)]
    unsafe fn end(&mut self) -> Self::SubmitInfo;
}

// Ignore for the moment (:
/*
// vk: primary/seconday | outside
fn set_event(&mut self); // vk: Graphics/Compute // d3d12:! emulation needed
// vk: primary/seconday | outside
fn reset_event(&mut self); // vk: Graphics/Compute
// vk: primary/seconday | inside/outside
fn wait_event(&mut self); // vk: Graphics/Compute

// vk: primary/seconday | inside/outside // d3d12: primary
fn begin_query(&mut self); // vk: Graphics/Compute // d3d12: BeginQuery
// vk: primary/seconday | inside/outside // d3d12: primary
fn end_query(&mut self); // vk: Graphics/Compute // d3d12: EndQuery
// vk: primary/seconday | outside
fn reset_query_pool(&mut self); // vk: Graphics/Compute
// vk: primary/seconday | inside/outside
fn write_timestamp(&mut self); // vk: Graphics/Compute
// vk: primary/seconday | outside
fn copy_query_pool_results(&mut self); // vk: Graphics/Compute
*/
