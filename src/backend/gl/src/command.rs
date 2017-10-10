#![allow(missing_docs)]

use gl;
use core::{self as c, command, image, memory, target, Viewport};
use core::buffer::IndexBufferView;
use core::target::{ColorValue, Stencil};
use {native as n, Backend};
use pool::{self, BufferMemory};

use std::mem;
use std::ops::Range;
use std::sync::{Arc, Mutex};

// Command buffer implementation details:
//
// The underlying commands and data are stored inside the associated command pool.
// See the comments for further safety requirements.
// Each command buffer holds a (growable) slice of the buffers in the pool.
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
#[derive(Clone, Debug)]
pub enum Command {
    Dispatch(u32, u32, u32),
    DispatchIndirect(gl::types::GLuint, u64),
    Draw {
        primitive: gl::types::GLenum,
        vertices: Range<c::VertexCount>,
        instances: Range<c::InstanceCount>,
    },
    DrawIndexed {
        primitive: gl::types::GLenum,
        index_type: gl::types::GLenum,
        index_count: c::IndexCount,
        index_buffer_offset: u64,
        base_vertex: c::VertexOffset,
        instances: Range<c::InstanceCount>,
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
    pub(crate) memory: Arc<Mutex<BufferMemory>>,
    pub(crate) buf: BufferSlice,
    // Buffer id for the owning command pool.
    // Only relevant if individual resets are allowed.
    pub(crate) id: u64,
    individual_reset: bool,

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
    pub(crate) fn new(
        fbo: n::FrameBuffer,
        limits: Limits,
        memory: Arc<Mutex<BufferMemory>>,
    ) -> Self {
        let (id, individual_reset) = {
            let mut memory = memory
                .try_lock()
                .expect("Trying to allocate a command buffers, while memory is in-use.");

            match *memory {
                BufferMemory::Linear(_) => (0, false),
                BufferMemory::Individual { ref mut storage, ref mut next_buffer_id } => {
                    // Add a new pair of buffers
                    storage.insert(*next_buffer_id, pool::OwnedBuffer::new());
                    let id = *next_buffer_id;
                    *next_buffer_id += 1;
                    (id, true)
                }
            }
        };

        RawCommandBuffer {
            memory,
            buf: BufferSlice::new(),
            id,
            individual_reset,
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
        let slice = {
            let mut memory = self
                .memory
                .try_lock()
                .expect("Trying to record a command buffers, while memory is in-use.");

            let cmd_buffer = match *memory {
                BufferMemory::Linear(ref mut buffer) => &mut buffer.commands,
                BufferMemory::Individual { ref mut storage, .. } => {
                    &mut storage.get_mut(&self.id).unwrap().commands
                }
            };
            cmd_buffer.push(cmd);
            BufferSlice {
                offset: cmd_buffer.len() as u32 - 1,
                size: 1,
            }
        };
        self.buf.append(slice);
    }

    /// Copy a given vector slice into the data buffer.
    fn add<T>(&mut self, data: &[T]) -> BufferSlice {
        self.add_raw(unsafe { mem::transmute(data) })
    }
    /// Copy a given u8 slice into the data buffer.
    fn add_raw(&mut self, data: &[u8]) -> BufferSlice {
        let mut memory = self
                .memory
                .try_lock()
                .expect("Trying to record a command buffers, while memory is in-use.");

        let data_buffer = match *memory {
            BufferMemory::Linear(ref mut buffer) => &mut buffer.data,
            BufferMemory::Individual { ref mut storage, .. } => {
                &mut storage.get_mut(&self.id).unwrap().data
            }
        };
        data_buffer.extend_from_slice(data);
        let slice = BufferSlice {
            offset: (data_buffer.len() - data.len()) as u32,
            size: data.len() as u32,
        };
        slice
    }

    fn is_main_target(&self, tv: n::TargetView) -> bool {
        tv == n::TargetView::Surface(0)
    }
}

impl command::RawCommandBuffer<Backend> for RawCommandBuffer {
    fn begin(&mut self) {
        // Implicit buffer reset when individual reset is set.
        if self.individual_reset {
            self.reset(false);
        }
    }

    fn finish(&mut self) {
        // no-op
    }

    fn reset(&mut self, _release_resources: bool) {
        if !self.individual_reset {
            error!("Associated pool must allow individual resets.");
            return
        }

        self.soft_reset();
        let mut memory = self
                .memory
                .try_lock()
                .expect("Trying to reset a command buffers, while memory is in-use.");

        match *memory {
            // Linear` can't have individual reset ability.
            BufferMemory::Linear(_) => unreachable!(),
            BufferMemory::Individual { ref mut storage, .. } => {
                // TODO: should use the `release_resources` and shrink the buffers?
                storage
                    .get_mut(&self.id)
                    .map(|buffer| {
                        buffer.commands.clear();
                        buffer.data.clear();
                    });
            }
        }

    }

    fn pipeline_barrier(
        &mut self,
        _stages: Range<c::pso::PipelineStage>,
        _barries: &[memory::Barrier<Backend>],
    ) {
        unimplemented!()
    }

    fn fill_buffer(&mut self, _buffer: &n::Buffer, _range: Range<u64>, _data: u32) {
        unimplemented!()
    }

    fn update_buffer(&mut self, _buffer: &n::Buffer, _offset: u64, _data: &[u8]) {
        unimplemented!()
    }

    fn begin_renderpass(
        &mut self,
        _render_pass: &n::RenderPass,
        _frame_buffer: &n::FrameBuffer,
        _render_area: target::Rect,
        _clear_values: &[command::ClearValue],
        _first_subpass: command::SubpassContents,
    ) {
        unimplemented!()
    }

    fn next_subpass(&mut self, _contents: command::SubpassContents) {
        unimplemented!()
    }

    fn end_renderpass(&mut self) {
        unimplemented!()
    }

    fn clear_color(
        &mut self,
        rtv: &n::TargetView,
        _: image::ImageLayout,
        value: command::ClearColor,
    ) {
        if self.is_main_target(*rtv) {
            let fbo = self.display_fb;
            self.push_cmd(Command::BindFrameBuffer(gl::DRAW_FRAMEBUFFER, fbo));
        } else {
            let fbo = self.fbo;
            self.push_cmd(Command::BindFrameBuffer(gl::DRAW_FRAMEBUFFER, fbo));
            self.push_cmd(Command::BindTargetView(gl::DRAW_FRAMEBUFFER, gl::COLOR_ATTACHMENT0, *rtv));
            self.push_cmd(Command::SetDrawColorBuffers(1));
        }

        self.push_cmd(Command::ClearColor(*rtv, value));
    }

    fn clear_depth_stencil(
        &mut self,
        _dsv: &n::TargetView,
        _: image::ImageLayout,
        _depth: Option<target::Depth>,
        _stencil: Option<target::Stencil>,
    ) {
        unimplemented!()
    }

    fn clear_attachments(&mut self, _: &[command::AttachmentClear], _: &[target::Rect]) {
        unimplemented!()
    }

    fn resolve_image(
        &mut self,
        _src: &n::Image,
        _src_layout: image::ImageLayout,
        _dst: &n::Image,
        _dst_layout: image::ImageLayout,
        _regions: &[command::ImageResolve],
    ) {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, ibv: IndexBufferView<Backend>) {
        // TODO: how can we incoporate the buffer offset?
        if ibv.offset > 0 {
            warn!("Non-zero index buffer offset currently not handled.");
        }

        self.cache.index_type = Some(ibv.index_type);
        self.push_cmd(Command::BindIndexBuffer(ibv.buffer.raw));
    }

    fn bind_vertex_buffers(&mut self, _vbs: c::pso::VertexBufferSet<Backend>) {
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

    fn bind_graphics_pipeline(&mut self, _pipeline: &n::GraphicsPipeline) {
        unimplemented!()
    }

    fn bind_graphics_descriptor_sets(
        &mut self,
        _layout: &n::PipelineLayout,
        _first_set: usize,
        _sets: &[&n::DescriptorSet],
    ) {
        unimplemented!()
    }

    fn bind_compute_pipeline(&mut self, _pipeline: &n::ComputePipeline) {
        unimplemented!()
    }

    fn bind_compute_descriptor_sets(
        &mut self,
        _layout: &n::PipelineLayout,
        _first_set: usize,
        _sets: &[&n::DescriptorSet],
    ) {
        unimplemented!()
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.push_cmd(Command::Dispatch(x, y, z));
    }

    fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: u64) {
        self.push_cmd(Command::DispatchIndirect(buffer.raw, offset));
    }

    fn copy_buffer(&mut self, _src: &n::Buffer, _dst: &n::Buffer, _regions: &[command::BufferCopy]) {
        unimplemented!()
    }

    fn copy_image(
        &mut self,
        _src: &n::Image,
        _src_layout: image::ImageLayout,
        _dst: &n::Image,
        _dst_layout: image::ImageLayout,
        _regions: &[command::ImageCopy],
    ) {
        unimplemented!()
    }

    fn copy_buffer_to_image(
        &mut self,
        _src: &n::Buffer,
        _dst: &n::Image,
        _dst_layout: image::ImageLayout,
        _regions: &[command::BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn copy_image_to_buffer(
        &mut self,
        _src: &n::Image,
        _src_layout: image::ImageLayout,
        _dst: &n::Buffer,
        _regions: &[command::BufferImageCopy],
    ) {
        unimplemented!()
    }

    fn draw(
        &mut self,
        vertices: Range<c::VertexCount>,
        instances: Range<c::InstanceCount>,
    ) {
        match self.cache.primitive {
            Some(primitive) => {
                self.push_cmd(
                    Command::Draw {
                        primitive,
                        vertices,
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
        indices: Range<c::IndexCount>,
        base_vertex: c::VertexOffset,
        instances: Range<c::InstanceCount>,
    ) {
        let (start, index_type) = match self.cache.index_type {
            Some(c::IndexType::U16) => (indices.start * 2, gl::UNSIGNED_SHORT),
            Some(c::IndexType::U32) => (indices.start * 4, gl::UNSIGNED_INT),
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
                        index_count: indices.end - indices.start,
                        index_buffer_offset: start as _,
                        base_vertex,
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

    fn draw_indirect(
        &mut self,
        _buffer: &n::Buffer,
        _offset: u64,
        _draw_count: u32,
        _stride: u32,
    ) {
        unimplemented!()
    }

    fn draw_indexed_indirect(
        &mut self,
        _buffer: &n::Buffer,
        _offset: u64,
        _draw_count: u32,
        _stride: u32,
    ) {
        unimplemented!()
    }
}

/// A subpass command buffer abstraction for OpenGL
pub struct SubpassCommandBuffer;
