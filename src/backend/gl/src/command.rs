#![allow(missing_docs)]

use gl;
use core::{self as c, command, memory, state as s, target, texture, Viewport};
use core::buffer::IndexBufferView;
use core::command::{BufferCopy, BufferImageCopy, ClearValue, ImageCopy, ImageResolve,
                    InstanceParams, SubpassContents};
use core::target::{ColorValue, Depth, Mirror, Rect, Stencil};
use {native as n, Backend};
use std::{mem, slice};

/// The place of some data in the data buffer.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DataPointer {
    offset: u32,
    size: u32,
}

impl DataPointer {
    // Append a data pointer, resulting in one data pointer
    // covering the whole memory region.
    fn append(&mut self, other: DataPointer) {
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

#[derive(Clone)]
pub struct DataBuffer(Vec<u8>);
impl DataBuffer {
    /// Create a new empty data buffer.
    pub fn new() -> DataBuffer {
        DataBuffer(Vec::new())
    }
    /// Copy a given vector slice into the buffer.
    fn add<T>(&mut self, data: &[T]) -> DataPointer {
        self.add_raw(unsafe { mem::transmute(data) })
    }
    /// Copy a given u8 slice into the buffer.
    fn add_raw(&mut self, data: &[u8]) -> DataPointer {
        self.0.extend_from_slice(data);
        DataPointer {
            offset: (self.0.len() - data.len()) as u32,
            size: data.len() as u32,
        }
    }
    /// Return a reference to a stored data object.
    pub fn get<T>(&self, ptr: DataPointer) -> &[T] {
        assert_eq!(ptr.size % mem::size_of::<T>() as u32, 0);
        let raw_data = self.get_raw(ptr);
        unsafe { mem::transmute(raw_data) }
    }
    /// Return a reference to a stored data object.
    pub fn get_raw(&self, ptr: DataPointer) -> &[u8] {
        &self.0[ptr.offset as usize..(ptr.offset + ptr.size) as usize]
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
    BindVertexBuffers(DataPointer),
    SetViewports {
        viewport_ptr: DataPointer,
        depth_range_ptr: DataPointer,
    },
    SetScissors(DataPointer),
}

#[allow(missing_copy_implementations)]
#[derive(Clone)]
pub struct SubmitInfo {
    // Raw pointer optimization:
    // Command buffers are stored inside the command pools.
    // We are using raw pointers here to avoid costly clones
    // and to circumvent the borrow checker. This is safe because
    // the command buffers are only reused after calling reset.
    // Reset also resets the command buffers and implies that all
    // submit infos are either consumed or thrown away.
    pub(crate) buf: *const Vec<Command>,
    pub(crate) data: *const DataBuffer,
}

// See the explanation above why this is safe.
unsafe impl Send for SubmitInfo {}

// Cache current states of the command buffer
struct Cache {
    // Active primitive topology, set by the current pipeline.
    primitive: Option<gl::types::GLenum>,
    // Active index type, set by the current index buffer.
    index_type: Option<c::IndexType>,
    // Indicates that invalid commands have been recorded.
    error_state: bool,
}

impl Cache {
    pub fn new() -> Cache {
        Cache {
            primitive: None,
            index_type: None,
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
pub struct RawCommandBuffer {
    pub buf: Vec<Command>,
    pub data: DataBuffer,
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

impl RawCommandBuffer {
    pub(crate) fn new(fbo: n::FrameBuffer, limits: Limits) -> Self {
        RawCommandBuffer {
            buf: Vec::new(),
            data: DataBuffer::new(),
            fbo,
            display_fb: 0 as n::FrameBuffer,
            cache: Cache::new(),
            limits,
            active_attribs: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        unimplemented!()
    }
}

impl command::RawCommandBuffer<Backend> for RawCommandBuffer {
    fn finish(&mut self) -> SubmitInfo {
        SubmitInfo {
            buf: &self.buf,
            data: &self.data,
        }
    }

    fn pipeline_barrier(&mut self, barries: &[memory::Barrier]) {
        unimplemented!()
    }

    fn begin_renderpass(
        &mut self,
        render_pass: &(),
        frame_buffer: &(),
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
        rtv: &n::TargetView,
        _: texture::ImageLayout,
        value: command::ClearColor,
    ) {
        unimplemented!()
    }

    fn clear_depth_stencil(
        &mut self,
        dsv: &n::TargetView,
        _: texture::ImageLayout,
        depth: Option<target::Depth>,
        stencil: Option<target::Stencil>,
    ) {
        unimplemented!()
    }

    fn resolve_image(
        &mut self,
        src: &n::Image,
        src_layout: texture::ImageLayout,
        dst: &n::Image,
        dst_layout: texture::ImageLayout,
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
        self.buf.push(Command::BindIndexBuffer(*ibv.buffer));
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
                let mut viewport_ptr = DataPointer { offset: 0, size: 0 };
                let mut depth_range_ptr = DataPointer { offset: 0, size: 0 };

                for viewport in viewports {
                    let viewport = &[viewport.x as f32, viewport.y as f32, viewport.w as f32, viewport.h as f32];
                    viewport_ptr.append(self.data.add::<f32>(viewport));
                }
                for viewport in viewports {
                    let depth_range = &[viewport.near as f64, viewport.far as f64];
                    depth_range_ptr.append(self.data.add::<f64>(depth_range));
                }
                self.buf.push(Command::SetViewports { viewport_ptr, depth_range_ptr });
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
                let mut scissors_ptr = DataPointer { offset: 0, size: 0 };
                for scissor in scissors {
                    let scissor = &[scissor.x as i32, scissor.y as i32, scissor.w as i32, scissor.h as i32];
                    scissors_ptr.append(self.data.add::<i32>(scissor));
                }
                self.buf.push(Command::SetScissors(scissors_ptr));
            }
            _ => {
                error!("Number of scissors exceeds the number of maximum viewports");
                self.cache.error_state = true;
            }
        }
    }

    fn set_ref_values(&mut self, rv: s::RefValues) {
        unimplemented!()
    }

    fn bind_descriptor_heap(&mut self, _: &n::DescriptorHeap) {
        // no-op, OpenGL doesn't have a concept of descriptor heaps
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &n::PipelineState) {
        unimplemented!()
    }

    fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &(),
        first_set: usize,
        sets: &[&n::DescriptorSet],
    ) {
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, pipeline: &n::PipelineState) {
        unimplemented!()
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.buf.push(Command::Dispatch(x, y, z));
    }

    fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: u64) {
        self.buf.push(Command::DispatchIndirect(*buffer, offset));
    }

    fn copy_buffer(&mut self, src: &n::Buffer, dst: &n::Buffer, regions: &[BufferCopy]) {
        unimplemented!()
    }

    fn copy_image(
        &mut self,
        src: &n::Image,
        src_layout: texture::ImageLayout,
        dst: &n::Image,
        dst_layout: texture::ImageLayout,
        regions: &[ImageCopy],
    ) {
        unimplemented!()
    }

    fn copy_buffer_to_image(
        &mut self,
        src: &n::Buffer,
        dst: &n::Image,
        layout: texture::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn copy_image_to_buffer(
        &mut self,
        src: &n::Image,
        dst: &n::Buffer,
        layout: texture::ImageLayout,
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
                self.buf.push(
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
                self.buf.push(
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
pub struct SubpassCommandBuffer {
    pub buf: Vec<Command>,
    pub data: DataBuffer,
}

impl SubpassCommandBuffer {
    pub fn new() -> Self {
        SubpassCommandBuffer {
            buf: Vec::new(),
            data: DataBuffer::new(),
        }
    }
}
