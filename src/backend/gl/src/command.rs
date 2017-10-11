#![allow(missing_docs)]

use gl;
use core::{self as c, command, image, memory, target, Viewport};
use core::buffer::IndexBufferView;
use core::command::{BufferCopy, BufferImageCopy, ClearValue, ImageCopy, ImageResolve,
                    InstanceParams, SubpassContents};
use core::target::{ColorValue, Stencil};
use {native as n, Backend};
use pool::{self, BufferMemory};
use std::cell::RefCell;
use std::mem;
use std::sync::Arc;

// Command buffer implementation details:
//
// The underlying commands and data are stored inside the associated command pool.
// See the comments for further safety requirements.
// Each command buffer holds a (growable) slice of the buffers (`CommandBuffer`
// and `DataBuffer`) in the pool.
//
// Command buffers are recorded one-after-another for each command pool.
// Actual storage depends on the resetting behavior of the pool.

/// The place of some data in a buffer.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BufferSlice {
    pub offset: u32,
    pub size: u32,
}

impl BufferSlice {
    fn new() -> Self {
        BufferSlice {
            offset: 0,
            size: 0,
        }
    }

    // Append a data pointer, resulting in one data pointer
    // covering the whole memory region.
    fn append(&mut self, other: BufferSlice) {
        if self.size == 0 {
            // Empty or dummy pointer
            self.offset = other.offset;
            self.size = other.size;
        } else {
            assert_eq!(self.offset + self.size, other.offset);
            self.size += other.size;
        }
    }
}

///
#[derive(Copy, Clone, Debug)]
pub enum Command {
    Dispatch(u32, u32, u32),
    DispatchIndirect(gl::types::GLuint, u64),
    Draw {
        primitive: gl::types::GLenum,
        start: c::VertexCount,
        count: c::VertexCount,
        instances: Option<InstanceParams>,
    },
    DrawIndexed {
        primitive: gl::types::GLenum,
        index_type: gl::types::GLenum,
        start: c::VertexCount,
        count: c::VertexCount,
        base: c::VertexOffset,
        instances: Option<InstanceParams>,
    },
    BindIndexBuffer(gl::types::GLuint),
    BindVertexBuffers(BufferSlice),
    SetViewports {
        viewport_ptr: BufferSlice,
        depth_range_ptr: BufferSlice,
    },
    SetScissors(BufferSlice),
    SetBlendColor(ColorValue),
    ClearColor(n::TargetView, command::ClearColor),
    BindFrameBuffer(FrameBufferTarget, n::FrameBuffer),
    BindTargetView(FrameBufferTarget, AttachmentPoint, n::TargetView),
    SetDrawColorBuffers(usize),
}

pub type FrameBufferTarget = gl::types::GLenum;
pub type AttachmentPoint = gl::types::GLenum;

// Cache current states of the command buffer
#[derive(Clone)]
struct Cache {
    // Active primitive topology, set by the current pipeline.
    primitive: Option<gl::types::GLenum>,
    // Active index type, set by the current index buffer.
    index_type: Option<c::IndexType>,
    // Stencil reference values (front, back).
    stencil_ref: Option<(Stencil, Stencil)>,
    // Blend color.
    blend_color: Option<ColorValue>,
    ///
    framebuffer: Option<(FrameBufferTarget, n::FrameBuffer)>,
    ///
    // Indicates that invalid commands have been recorded.
    error_state: bool,
}

impl Cache {
    pub fn new() -> Cache {
        Cache {
            primitive: None,
            index_type: None,
            stencil_ref: None,
            blend_color: None,
            framebuffer: None,
            error_state: false,
        }
    }
}

// This is a subset of the device limits stripped down to the ones needed
// for command buffer validation.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    max_viewports: usize,
}

impl From<c::Limits> for Limits {
    fn from(l: c::Limits) -> Self {
        Limits {
            max_viewports: l.max_viewports,
        }
    }
}

/// A command buffer abstraction for OpenGL.
///
/// If you want to display your rendered results to a framebuffer created externally, see the
/// `display_fb` field.
#[derive(Clone)]
pub struct RawCommandBuffer {
    pub(crate) memory: Arc<RefCell<pool::OwnedBuffer>>,
    pub(crate) buf: BufferSlice,
    // Buffer id for the owning command pool.
    // Only relevant if individual resets are allowed.
    pub(crate) id: u64,

    fbo: n::FrameBuffer,
    /// The framebuffer to use for rendering to the main targets (0 by default).
    ///
    /// Use this to set the framebuffer that will be used for the screen display targets created
    /// with `create_main_targets_raw`. Usually you don't need to set this field directly unless
    /// your OS doesn't provide a default framebuffer with name 0 and you have to render to a
    /// different framebuffer object that can be made visible on the screen (iOS/tvOS need this).
    ///
    /// This framebuffer must exist and be configured correctly (with renderbuffer attachments,
    /// etc.) so that rendering to it can occur immediately.
    pub display_fb: n::FrameBuffer,
    cache: Cache,
    limits: Limits,
    active_attribs: usize,
}

unsafe impl Send for RawCommandBuffer { }

impl RawCommandBuffer {
    pub(crate) fn new(
        fbo: n::FrameBuffer,
        limits: Limits,
        memory: &mut BufferMemory,
    ) -> Self {
        let (memory, id) = match *memory {
            BufferMemory::Linear(ref buffer) => {
                (buffer.clone(), 0)
            }
            BufferMemory::Individual { ref mut storage, ref mut next_buffer_id } => {
                // Add a new pair of buffers
                let buffer = Arc::new(RefCell::new(pool::OwnedBuffer::new()));
                storage.insert(*next_buffer_id, buffer.clone());
                let id = *next_buffer_id;
                *next_buffer_id += 1;
                (buffer, id)
            }
        };

        RawCommandBuffer {
            memory,
            buf: BufferSlice::new(),
            id,
            fbo,
            display_fb: 0 as n::FrameBuffer,
            cache: Cache::new(),
            limits,
            active_attribs: 0,
        }
    }

    // Soft reset only the buffers, but doesn't free any memory or clears memory
    // of the owning pool.
    pub(crate) fn soft_reset(&mut self) {
        self.buf = BufferSlice::new();
        self.cache = Cache::new();
    }

    fn push_cmd(&mut self, cmd: Command) {
        let mut cmd_buffer = &mut self.memory.borrow_mut().commands;
        cmd_buffer.push(cmd);
        self.buf.append(
            BufferSlice {
                offset: cmd_buffer.len() as u32 - 1,
                size: 1,
            });
    }

    /// Copy a given vector slice into the data buffer.
    fn add<T>(&mut self, data: &[T]) -> BufferSlice {
        self.add_raw(unsafe { mem::transmute(data) })
    }
    /// Copy a given u8 slice into the data buffer.
    fn add_raw(&mut self, data: &[u8]) -> BufferSlice {
        let mut data_buffer = &mut self.memory.borrow_mut().data;
        data_buffer.extend_from_slice(data);
        BufferSlice {
            offset: (data_buffer.len() - data.len()) as u32,
            size: data.len() as u32,
        }
    }

    fn is_main_target(&self, tv: n::TargetView) -> bool {
        tv == n::TargetView::Surface(0)
    }
}

impl command::RawCommandBuffer<Backend> for RawCommandBuffer {
    fn begin(&mut self) {
        // no-op
    }

    fn finish(&mut self) {
        // no-op
    }

    fn reset(&mut self, _release_resources: bool) {
        // TODO: error when calling this for linear memory storage
        //       Currently works under the assumption that the user
        //       calls this function API conform.
        // TODO: should use the `release_resources` and shrink the buffers?
        self.soft_reset();
        let mut buffer = self.memory.borrow_mut();
        buffer.commands.clear();
        buffer.data.clear();
    }

    fn pipeline_barrier(&mut self, barries: &[memory::Barrier<Backend>]) {
        unimplemented!()
    }

    fn begin_renderpass(
        &mut self,
        render_pass: &n::RenderPass,
        frame_buffer: &n::FrameBuffer,
        render_area: target::Rect,
        clear_values: &[ClearValue],
        first_subpass: SubpassContents,
    ) {
        unimplemented!()
    }

    fn next_subpass(&mut self, contents: SubpassContents) {
        unimplemented!()
    }

    fn end_renderpass(&mut self) {
        unimplemented!()
    }

    fn clear_color(
        &mut self,
        rtv: &n::RenderTargetView,
        _: image::ImageLayout,
        value: command::ClearColor,
    ) {
        if self.is_main_target(rtv.view) {
            let fbo = self.display_fb;
            self.push_cmd(Command::BindFrameBuffer(gl::DRAW_FRAMEBUFFER, fbo));
        } else {
            let fbo = self.fbo;
            self.push_cmd(Command::BindFrameBuffer(gl::DRAW_FRAMEBUFFER, fbo));
            self.push_cmd(Command::BindTargetView(gl::DRAW_FRAMEBUFFER, gl::COLOR_ATTACHMENT0, rtv.view));
            self.push_cmd(Command::SetDrawColorBuffers(1));
        }

        self.push_cmd(Command::ClearColor(rtv.view, value));
    }

    fn clear_depth_stencil(
        &mut self,
        dsv: &n::DepthStencilView,
        _: image::ImageLayout,
        depth: Option<target::Depth>,
        stencil: Option<target::Stencil>,
    ) {
        unimplemented!()
    }

    fn resolve_image(
        &mut self,
        src: &n::Image,
        src_layout: image::ImageLayout,
        dst: &n::Image,
        dst_layout: image::ImageLayout,
        regions: &[ImageResolve],
    ) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<Backend>) {
        // TODO: how can we incoporate the buffer offset?
        if ibv.offset > 0 {
            warn!("Non-zero index buffer offset currently not handled.");
        }

        self.cache.index_type = Some(ibv.index_type);
        self.push_cmd(Command::BindIndexBuffer(*ibv.buffer));
    }

    fn bind_vertex_buffers(&mut self, vbs: c::pso::VertexBufferSet<Backend>) {
        unimplemented!()
    }

    fn set_viewports(&mut self, viewports: &[Viewport]) {
        match viewports.len() {
            0 => {
                error!("Number of viewports can not be zero.");
                self.cache.error_state = true;
            }
            n if n <= self.limits.max_viewports => {
                // OpenGL has two functions for setting the viewports.
                // Configuring the rectangle area and setting the depth bounds are separated.
                //
                // We try to store everything into a contiguous block of memory,
                // which allows us to avoid memory allocations when executing the commands.
                let mut viewport_ptr = BufferSlice { offset: 0, size: 0 };
                let mut depth_range_ptr = BufferSlice { offset: 0, size: 0 };

                for viewport in viewports {
                    let viewport = &[viewport.x as f32, viewport.y as f32, viewport.w as f32, viewport.h as f32];
                    viewport_ptr.append(self.add::<f32>(viewport));
                }
                for viewport in viewports {
                    let depth_range = &[viewport.near as f64, viewport.far as f64];
                    depth_range_ptr.append(self.add::<f64>(depth_range));
                }
                self.push_cmd(Command::SetViewports { viewport_ptr, depth_range_ptr });
            }
            _ => {
                error!("Number of viewports exceeds the number of maximum viewports");
                self.cache.error_state = true;
            }
        }
    }

    fn set_scissors(&mut self, scissors: &[target::Rect]) {
        match scissors.len() {
            0 => {
                error!("Number of scissors can not be zero.");
                self.cache.error_state = true;
            }
            n if n <= self.limits.max_viewports => {
                let mut scissors_ptr = BufferSlice { offset: 0, size: 0 };
                for scissor in scissors {
                    let scissor = &[scissor.x as i32, scissor.y as i32, scissor.w as i32, scissor.h as i32];
                    scissors_ptr.append(self.add::<i32>(scissor));
                }
                self.push_cmd(Command::SetScissors(scissors_ptr));
            }
            _ => {
                error!("Number of scissors exceeds the number of maximum viewports");
                self.cache.error_state = true;
            }
        }
    }

    fn set_stencil_reference(&mut self, front: target::Stencil, back: target::Stencil) {
        // Only cache the stencil references values until
        // we assembled all the pieces to set the stencil state
        // from the pipeline.
        self.cache.stencil_ref = Some((front, back));
    }

    fn set_blend_constants(&mut self, cv: target::ColorValue) {
        if self.cache.blend_color != Some(cv) {
            self.cache.blend_color = Some(cv);
            self.push_cmd(Command::SetBlendColor(cv));
        }
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &n::GraphicsPipeline) {
        unimplemented!()
    }

    fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: &[&n::DescriptorSet],
    ) {
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, pipeline: &n::ComputePipeline) {
        unimplemented!()
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.push_cmd(Command::Dispatch(x, y, z));
    }

    fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: u64) {
        self.push_cmd(Command::DispatchIndirect(*buffer, offset));
    }

    fn copy_buffer(&mut self, src: &n::Buffer, dst: &n::Buffer, regions: &[BufferCopy]) {
        unimplemented!()
    }

    fn copy_image(
        &mut self,
        src: &n::Image,
        src_layout: image::ImageLayout,
        dst: &n::Image,
        dst_layout: image::ImageLayout,
        regions: &[ImageCopy],
    ) {
        unimplemented!()
    }

    fn copy_buffer_to_image(
        &mut self,
        src: &n::Buffer,
        dst: &n::Image,
        layout: image::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn copy_image_to_buffer(
        &mut self,
        src: &n::Image,
        dst: &n::Buffer,
        layout: image::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn draw(
        &mut self,
        start: c::VertexCount,
        count: c::VertexCount,
        instances: Option<InstanceParams>,
    ) {
        match self.cache.primitive {
            Some(primitive) => {
                self.push_cmd(
                    Command::Draw {
                        primitive,
                        start,
                        count,
                        instances,
                    }
                );
            }
            None => {
                warn!("No primitive bound. An active pipeline needs to be bound before calling `draw`.");
                self.cache.error_state = true;
            }
        }
    }

    fn draw_indexed(
        &mut self,
        start: c::VertexCount,
        count: c::VertexCount,
        base: c::VertexOffset,
        instances: Option<InstanceParams>,
    ) {
        let (start, index_type) = match self.cache.index_type {
            Some(c::IndexType::U16) => (start * 2u32, gl::UNSIGNED_SHORT),
            Some(c::IndexType::U32) => (start * 4u32, gl::UNSIGNED_INT),
            None => {
                warn!("No index type bound. An index buffer needs to be bound before calling `draw_indexed`.");
                self.cache.error_state = true;
                return;
            }
        };
        match self.cache.primitive {
            Some(primitive) => {
                self.push_cmd(
                    Command::DrawIndexed {
                        primitive,
                        index_type,
                        start,
                        count,
                        base,
                        instances,
                    }
                );
            }
            None => {
                warn!("No primitive bound. An active pipeline needs to be bound before calling `draw_indexed`.");
                self.cache.error_state = true;
            }
        }
    }

    fn draw_indirect(&mut self, buffer: &n::Buffer, offset: u64, draw_count: u32, stride: u32) {
        unimplemented!()
    }

    fn draw_indexed_indirect(
        &mut self,
        buffer: &n::Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    ) {
        unimplemented!()
    }
}

/// A subpass command buffer abstraction for OpenGL
#[allow(missing_copy_implementations)]
pub struct SubpassCommandBuffer;
