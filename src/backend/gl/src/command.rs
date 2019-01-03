#![allow(missing_docs)]

use crate::{gl, GlContext};

use crate::hal::format::ChannelType;
use crate::hal::range::RangeArg;
use crate::hal::{self, buffer, command, image, memory, pass, pso, query, ColorSlot};

use crate::pool::{self, BufferMemory};
use crate::{native as n, Backend};

use std::borrow::Borrow;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::{mem, slice};

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
        BufferSlice { offset: 0, size: 0 }
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
#[derive(Debug)]
pub enum Command {
    Dispatch(hal::WorkGroupCount),
    DispatchIndirect(gl::types::GLuint, buffer::Offset),
    Draw {
        primitive: u32,
        vertices: Range<hal::VertexCount>,
        instances: Range<hal::InstanceCount>,
    },
    DrawIndexed {
        primitive: u32,
        index_type: u32,
        index_count: hal::IndexCount,
        index_buffer_offset: buffer::Offset,
        base_vertex: hal::VertexOffset,
        instances: Range<hal::InstanceCount>,
    },
    BindIndexBuffer(<GlContext as glow::Context>::Buffer),
    //BindVertexBuffers(BufferSlice),
    BindUniform {
        uniform: n::UniformDesc,
        buffer: BufferSlice,
    },
    BindRasterizer {
        rasterizer: pso::Rasterizer,
    },
    BindDepth {
        depth: pso::DepthTest,
    },
    SetViewports {
        first_viewport: u32,
        viewport_ptr: BufferSlice,
        depth_range_ptr: BufferSlice,
    },
    SetScissors(u32, BufferSlice),
    SetBlendColor(pso::ColorValue),

    /// Clear floating-point color drawbuffer of bound framebuffer.
    ClearBufferColorF(DrawBuffer, [f32; 4]),
    /// Clear unsigned integer color drawbuffer of bound framebuffer.
    ClearBufferColorU(DrawBuffer, [u32; 4]),
    /// Clear signed integer color drawbuffer of bound framebuffer.
    ClearBufferColorI(DrawBuffer, [i32; 4]),
    /// Clear depth-stencil drawbuffer of bound framebuffer.
    ClearBufferDepthStencil(Option<pso::DepthValue>, Option<pso::StencilValue>),
    /// Clear the currently bound texture with the given color.
    ClearTexture([f32; 4]),

    /// Set list of color attachments for drawing.
    /// The buffer slice contains a list of `GLenum`.
    DrawBuffers(BufferSlice),

    BindFrameBuffer(FrameBufferTarget, Option<n::FrameBuffer>),
    BindTargetView(FrameBufferTarget, AttachmentPoint, n::ImageView),
    SetDrawColorBuffers(usize),
    SetPatchSize(i32),
    BindProgram(<GlContext as glow::Context>::Program),
    BindBlendSlot(ColorSlot, pso::ColorBlendDesc),
    BindAttribute(
        n::AttributeDesc,
        gl::types::GLuint,
        gl::types::GLsizei,
        gl::types::GLuint,
    ),
    //UnbindAttribute(n::AttributeDesc),
    CopyBufferToBuffer(n::RawBuffer, n::RawBuffer, command::BufferCopy),
    CopyBufferToTexture(n::RawBuffer, n::Texture, command::BufferImageCopy),
    CopyBufferToSurface(n::RawBuffer, n::Surface, command::BufferImageCopy),
    CopyTextureToBuffer(n::Texture, n::RawBuffer, command::BufferImageCopy),
    CopySurfaceToBuffer(n::Surface, n::RawBuffer, command::BufferImageCopy),
    CopyImageToTexture(n::ImageKind, n::Texture, command::ImageCopy),
    CopyImageToSurface(n::ImageKind, n::Surface, command::ImageCopy),

    BindBufferRange(u32, u32, n::RawBuffer, i32, i32),
    BindTexture(u32, n::Texture),
    BindSampler(u32, n::Texture),
    SetTextureSamplerSettings(gl::types::GLuint, n::Texture, image::SamplerInfo),
}

pub type FrameBufferTarget = u32;
pub type AttachmentPoint = gl::types::GLenum;
pub type DrawBuffer = u32;

#[derive(Clone, Debug)]
struct AttachmentClear {
    subpass_id: Option<pass::SubpassId>,
    value: Option<command::ClearValueRaw>,
    stencil_value: Option<pso::StencilValue>,
}

#[derive(Debug)]
pub struct RenderPassCache {
    render_pass: n::RenderPass,
    framebuffer: n::FrameBuffer,
    attachment_clears: Vec<AttachmentClear>,
}

// Cache current states of the command buffer
#[derive(Debug)]
struct Cache {
    // Active primitive topology, set by the current pipeline.
    primitive: Option<u32>,
    // Active index type, set by the current index buffer.
    index_type: Option<hal::IndexType>,
    // Stencil reference values (front, back).
    stencil_ref: Option<(pso::StencilValue, pso::StencilValue)>,
    // Blend color.
    blend_color: Option<pso::ColorValue>,
    ///
    framebuffer: Option<(FrameBufferTarget, n::FrameBuffer)>,
    ///
    // Indicates that invalid commands have been recorded.
    error_state: bool,
    // Vertices per patch for tessellation primitives (patches).
    patch_size: Option<gl::types::GLint>,
    // Active program name.
    program: Option<gl::types::GLuint>,
    // Blend per attachment.
    blend_targets: Option<Vec<Option<pso::ColorBlendDesc>>>,
    // Maps bound vertex buffer offset (index) to handle.
    vertex_buffers: Vec<gl::types::GLuint>,
    // Active vertex buffer descriptions.
    vertex_buffer_descs: Vec<Option<pso::VertexBufferDesc>>,
    // Active attributes.
    attributes: Vec<n::AttributeDesc>,
    // Active uniforms
    uniforms: Vec<n::UniformDesc>,
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
            patch_size: None,
            program: None,
            blend_targets: None,
            vertex_buffers: Vec::new(),
            vertex_buffer_descs: Vec::new(),
            attributes: Vec::new(),
            uniforms: Vec::new(),
        }
    }
}

// This is a subset of the device limits stripped down to the ones needed
// for command buffer validation.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    max_viewports: usize,
}

impl From<hal::Limits> for Limits {
    fn from(l: hal::Limits) -> Self {
        Limits {
            max_viewports: l.max_viewports,
        }
    }
}

/// A command buffer abstraction for OpenGL.
///
/// If you want to display your rendered results to a framebuffer created externally, see the
/// `display_fb` field.
#[derive(Debug)]
pub struct RawCommandBuffer {
    pub(crate) memory: Arc<Mutex<BufferMemory>>,
    pub(crate) buf: BufferSlice,
    // Buffer id for the owning command pool.
    // Only relevant if individual resets are allowed.
    pub(crate) id: u64,
    individual_reset: bool,

    fbo: Option<n::FrameBuffer>,
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

    pass_cache: Option<RenderPassCache>,
    cur_subpass: usize,

    limits: Limits,
    active_attribs: usize,
}

impl RawCommandBuffer {
    pub(crate) fn new(
        fbo: Option<n::FrameBuffer>,
        limits: Limits,
        memory: Arc<Mutex<BufferMemory>>,
    ) -> Self {
        let (id, individual_reset) = {
            let mut memory = memory
                .try_lock()
                .expect("Trying to allocate a command buffers, while memory is in-use.");

            match *memory {
                BufferMemory::Linear(_) => (0, false),
                BufferMemory::Individual {
                    ref mut storage,
                    ref mut next_buffer_id,
                } => {
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
            pass_cache: None,
            cur_subpass: !0,
            limits,
            active_attribs: 0,
        }
    }

    // Soft reset only the buffers, but doesn't free any memory or clears memory
    // of the owning pool.
    pub(crate) fn soft_reset(&mut self) {
        self.buf = BufferSlice::new();
        self.cache = Cache::new();
        self.pass_cache = None;
        self.cur_subpass = !0;
    }

    fn push_cmd(&mut self, cmd: Command) {
        push_cmd_internal(&self.id, &mut self.memory, &mut self.buf, cmd);
    }

    /// Copy a given vector slice into the data buffer.
    fn add<T>(&mut self, data: &[T]) -> BufferSlice {
        self.add_raw(unsafe {
            slice::from_raw_parts(data.as_ptr() as *const _, data.len() * mem::size_of::<T>())
        })
    }

    /// Copy a given u8 slice into the data buffer.
    fn add_raw(&mut self, data: &[u8]) -> BufferSlice {
        let mut memory = self
            .memory
            .try_lock()
            .expect("Trying to record a command buffers, while memory is in-use.");

        let data_buffer = match *memory {
            BufferMemory::Linear(ref mut buffer) => &mut buffer.data,
            BufferMemory::Individual {
                ref mut storage, ..
            } => &mut storage.get_mut(&self.id).unwrap().data,
        };
        data_buffer.extend_from_slice(data);
        let slice = BufferSlice {
            offset: (data_buffer.len() - data.len()) as u32,
            size: data.len() as u32,
        };
        slice
    }

    fn update_blend_targets(&mut self, blend_targets: &Vec<pso::ColorBlendDesc>) {
        let max_blend_slots = blend_targets.len();

        if max_blend_slots > 0 {
            match self.cache.blend_targets {
                Some(ref mut cached) => {
                    if cached.len() < max_blend_slots {
                        cached.resize(max_blend_slots, None);
                    }
                }
                None => {
                    self.cache.blend_targets = Some(vec![None; max_blend_slots]);
                }
            };
        }

        for (slot, blend_target) in blend_targets.iter().enumerate() {
            let mut update_blend = false;
            if let Some(ref mut cached_targets) = self.cache.blend_targets {
                if let Some(cached_target) = cached_targets.get(slot) {
                    match cached_target {
                        &Some(ref cache) => {
                            if cache != blend_target {
                                update_blend = true;
                            }
                        }
                        &None => {
                            update_blend = true;
                        }
                    }
                }

                if update_blend {
                    cached_targets[slot] = Some(*blend_target);
                }
            }

            if update_blend {
                self.push_cmd(Command::BindBlendSlot(slot as _, *blend_target));
            }
        }
    }

    pub(crate) fn bind_attributes(&mut self) {
        let Cache {
            ref attributes,
            ref vertex_buffers,
            ref vertex_buffer_descs,
            ..
        } = self.cache;

        for attribute in attributes {
            let binding = attribute.binding as usize;

            if vertex_buffers.len() <= binding {
                error!("No vertex buffer bound at {}", binding);
            }

            let handle = vertex_buffers[binding];

            match vertex_buffer_descs.get(binding) {
                Some(&Some(desc)) => {
                    push_cmd_internal(
                        &self.id,
                        &mut self.memory,
                        &mut self.buf,
                        Command::BindAttribute(
                            attribute.clone(),
                            handle,
                            desc.stride as _,
                            desc.rate.as_uint() as u32,
                        ),
                    );
                }
                _ => error!("No vertex buffer description bound at {}", binding),
            }
        }
    }

    fn begin_subpass(&mut self) {
        // Split processing and command recording due to borrowchk.
        let (draw_buffers, clear_cmds) = {
            let state = self.pass_cache.as_ref().unwrap();
            let subpass = &state.render_pass.subpasses[self.cur_subpass];

            // See `begin_renderpass_cache` for clearing strategy

            // Bind draw buffers for mapping color output locations with
            // framebuffer attachments.
            let draw_buffers = if state.framebuffer == n::DEFAULT_FRAMEBUFFER {
                // The default framebuffer is created by the driver
                // We don't have influence on its layout and we treat it as single image.
                //
                // TODO: handle case where we don't du double-buffering?
                vec![gl::BACK_LEFT]
            } else {
                subpass
                    .color_attachments
                    .iter()
                    .map(|id| gl::COLOR_ATTACHMENT0 + *id as gl::types::GLenum)
                    .collect::<Vec<_>>()
            };

            let clear_cmds = state
                .render_pass
                .attachments
                .iter()
                .zip(state.attachment_clears.iter())
                .filter_map(|(attachment, clear)| {
                    // Check if the attachment is first used in this subpass
                    if clear.subpass_id != Some(self.cur_subpass) {
                        return None;
                    }

                    // View format needs to be known at this point.
                    // All attachments specified in the renderpass must have a valid,
                    // matching image view bound in the framebuffer.
                    let view_format = attachment.format.unwrap();

                    // Clear color target
                    if view_format.is_color() {
                        if let Some(cv) = clear.value {
                            let channel = view_format.base_format().1;

                            let cmd = match channel {
                                ChannelType::Unorm
                                | ChannelType::Snorm
                                | ChannelType::Ufloat
                                | ChannelType::Sfloat
                                | ChannelType::Srgb
                                | ChannelType::Uscaled
                                | ChannelType::Sscaled => {
                                    Command::ClearBufferColorF(0, unsafe { cv.color.float32 })
                                }
                                ChannelType::Uint => {
                                    Command::ClearBufferColorU(0, unsafe { cv.color.uint32 })
                                }
                                ChannelType::Sint => {
                                    Command::ClearBufferColorI(0, unsafe { cv.color.int32 })
                                }
                            };

                            return Some(cmd);
                        }
                    }

                    // Clear depth-stencil target
                    let depth = if view_format.is_depth() {
                        clear.value.map(|cv| unsafe { cv.depth_stencil.depth })
                    } else {
                        None
                    };

                    let stencil = if view_format.is_stencil() {
                        clear.stencil_value
                    } else {
                        None
                    };

                    if depth.is_some() || stencil.is_some() {
                        return Some(Command::ClearBufferDepthStencil(depth, stencil));
                    }

                    None
                })
                .collect::<Vec<_>>();

            (draw_buffers, clear_cmds)
        };

        // Record commands
        let draw_buffers = self.add(&draw_buffers);
        self.push_cmd(Command::DrawBuffers(draw_buffers));

        for cmd in clear_cmds {
            self.push_cmd(cmd);
        }
    }
}

impl command::RawCommandBuffer<Backend> for RawCommandBuffer {
    unsafe fn begin(
        &mut self,
        _flags: hal::command::CommandBufferFlags,
        _inheritance_info: hal::command::CommandBufferInheritanceInfo<Backend>,
    ) {
        // TODO: Implement flags!
        if self.individual_reset {
            // Implicit buffer reset when individual reset is set.
            self.reset(false);
        } else {
            self.soft_reset();
        }
    }

    unsafe fn finish(&mut self) {
        // no-op
    }

    unsafe fn reset(&mut self, _release_resources: bool) {
        if !self.individual_reset {
            error!("Associated pool must allow individual resets.");
            return;
        }

        self.soft_reset();
        let mut memory = self
            .memory
            .try_lock()
            .expect("Trying to reset a command buffer, while memory is in-use.");

        match *memory {
            // Linear` can't have individual reset ability.
            BufferMemory::Linear(_) => unreachable!(),
            BufferMemory::Individual {
                ref mut storage, ..
            } => {
                // TODO: should use the `release_resources` and shrink the buffers?
                storage.get_mut(&self.id).map(|buffer| {
                    buffer.commands.clear();
                    buffer.data.clear();
                });
            }
        }
    }

    unsafe fn pipeline_barrier<'a, T>(
        &mut self,
        _stages: Range<hal::pso::PipelineStage>,
        _dependencies: memory::Dependencies,
        _barriers: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        // TODO
    }

    unsafe fn fill_buffer<R>(&mut self, _buffer: &n::Buffer, _range: R, _data: u32)
    where
        R: RangeArg<buffer::Offset>,
    {
        unimplemented!()
    }

    unsafe fn update_buffer(&mut self, _buffer: &n::Buffer, _offset: buffer::Offset, _data: &[u8]) {
        unimplemented!()
    }

    unsafe fn begin_render_pass<T>(
        &mut self,
        render_pass: &n::RenderPass,
        framebuffer: &n::FrameBuffer,
        _render_area: pso::Rect,
        clear_values: T,
        _first_subpass: command::SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::ClearValueRaw>,
    {
        // TODO: load ops: clearing strategy
        //  1.  < GL 3.0 / GL ES 2.0: glClear, only single color attachment?
        //  2.  = GL ES 2.0: glBindFramebuffer + glClear (no single draw buffer supported)
        //  3. >= GL 3.0 / GL ES 3.0: glBindFramerbuffer + glClearBuffer
        //
        // Clearing when entering a subpass:
        //    * Acquire channel information from renderpass description to
        //      select correct ClearBuffer variant.
        //    * Check for attachment loading clearing strategy

        // TODO: store ops:
        //   < GL 4.5: Ignore
        //  >= GL 4.5: Invalidate framebuffer attachment when store op is `DONT_CARE`.

        // 2./3.
        self.push_cmd(Command::BindFrameBuffer(glow::DRAW_FRAMEBUFFER, Some(*framebuffer)));

        let mut clear_values_iter = clear_values.into_iter();
        let attachment_clears = render_pass
            .attachments
            .iter()
            .enumerate()
            .map(|(i, attachment)| {
                let cv = if attachment.has_clears() {
                    Some(clear_values_iter.next().unwrap())
                } else {
                    None
                };
                AttachmentClear {
                    subpass_id: render_pass.subpasses.iter().position(|sp| sp.is_using(i)),
                    value: if attachment.ops.load == pass::AttachmentLoadOp::Clear {
                        Some(*cv.as_ref().unwrap().borrow())
                    } else {
                        None
                    },
                    stencil_value: if attachment.stencil_ops.load == pass::AttachmentLoadOp::Clear {
                        Some(cv.unwrap().borrow().depth_stencil.stencil)
                    } else {
                        None
                    },
                }
            })
            .collect();

        self.pass_cache = Some(RenderPassCache {
            render_pass: render_pass.clone(),
            framebuffer: *framebuffer,
            attachment_clears,
        });

        // Enter first subpass
        self.cur_subpass = 0;
        self.begin_subpass();
    }

    unsafe fn next_subpass(&mut self, _contents: command::SubpassContents) {
        unimplemented!()
    }

    unsafe fn end_render_pass(&mut self) {
        // TODO
    }

    unsafe fn clear_image<T>(
        &mut self,
        image: &n::Image,
        _: image::Layout,
        color: command::ClearColorRaw,
        _depth_stencil: command::ClearDepthStencilRaw,
        _subresource_ranges: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<image::SubresourceRange>,
    {
        // TODO: clearing strategies
        //  1.  < GL 3.0 / GL ES 3.0: glClear
        //  2.  < GL 4.4: glClearBuffer
        //  3. >= GL 4.4: glClearTexSubImage

        match self.fbo {
            Some(fbo) => {
                // TODO: reset color mask
                // 2. ClearBuffer
                let view = match image.kind {
                    n::ImageKind::Surface(id) => n::ImageView::Surface(id),
                    n::ImageKind::Texture(id) => n::ImageView::Texture(id, 0), //TODO
                };
                self.push_cmd(Command::BindFrameBuffer(
                    glow::DRAW_FRAMEBUFFER,
                    Some(fbo),
                ));
                self.push_cmd(Command::BindTargetView(
                    glow::DRAW_FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    view,
                ));
                self.push_cmd(Command::SetDrawColorBuffers(1));

                match image.channel {
                    ChannelType::Unorm
                    | ChannelType::Snorm
                    | ChannelType::Ufloat
                    | ChannelType::Sfloat
                    | ChannelType::Srgb
                    | ChannelType::Uscaled
                    | ChannelType::Sscaled => {
                        self.push_cmd(Command::ClearBufferColorF(0, color.float32))
                    }
                    ChannelType::Uint => self.push_cmd(Command::ClearBufferColorU(0, color.uint32)),
                    ChannelType::Sint => self.push_cmd(Command::ClearBufferColorI(0, color.int32)),
                }
            }
            None => {
                // 1. glClear
                let text = match image.kind {
                    n::ImageKind::Texture(id) => id, //TODO
                    n::ImageKind::Surface(_id) => unimplemented!(),
                };

                self.push_cmd(Command::BindTexture(0, text));
                self.push_cmd(Command::ClearTexture(color.float32));
            }
        }
    }

    unsafe fn clear_attachments<T, U>(&mut self, _: T, _: U)
    where
        T: IntoIterator,
        T::Item: Borrow<command::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
    {
        unimplemented!()
    }

    unsafe fn resolve_image<T>(
        &mut self,
        _src: &n::Image,
        _src_layout: image::Layout,
        _dst: &n::Image,
        _dst_layout: image::Layout,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::ImageResolve>,
    {
        unimplemented!()
    }

    unsafe fn blit_image<T>(
        &mut self,
        _src: &n::Image,
        _src_layout: image::Layout,
        _dst: &n::Image,
        _dst_layout: image::Layout,
        _filter: image::Filter,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::ImageBlit>,
    {
        unimplemented!()
    }

    unsafe fn bind_index_buffer(&mut self, ibv: buffer::IndexBufferView<Backend>) {
        // TODO: how can we incorporate the buffer offset?
        if ibv.offset > 0 {
            warn!("Non-zero index buffer offset currently not handled.");
        }

        self.cache.index_type = Some(ibv.index_type);
        self.push_cmd(Command::BindIndexBuffer(ibv.buffer.raw));
    }

    unsafe fn bind_vertex_buffers<I, T>(&mut self, first_binding: pso::BufferIndex, buffers: I)
    where
        I: IntoIterator<Item = (T, buffer::Offset)>,
        T: Borrow<n::Buffer>,
    {
        for (i, (buffer, offset)) in buffers.into_iter().enumerate() {
            let index = first_binding as usize + i;
            if self.cache.vertex_buffers.len() <= index {
                self.cache.vertex_buffers.resize(index + 1, 0);
            }
            self.cache.vertex_buffers[index] = buffer.borrow().raw;
            if offset != 0 {
                error!("Vertex buffer offset {} is not supported", offset);
            }
        }
    }

    unsafe fn set_viewports<T>(&mut self, first_viewport: u32, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        // OpenGL has two functions for setting the viewports.
        // Configuring the rectangle area and setting the depth bounds are separated.
        //
        // We try to store everything into a contiguous block of memory,
        // which allows us to avoid memory allocations when executing the commands.
        let mut viewport_ptr = BufferSlice { offset: 0, size: 0 };
        let mut depth_range_ptr = BufferSlice { offset: 0, size: 0 };

        let mut len = 0;
        for viewport in viewports {
            let viewport = viewport.borrow();
            let viewport_rect = &[
                viewport.rect.x as f32,
                viewport.rect.y as f32,
                viewport.rect.w as f32,
                viewport.rect.h as f32,
            ];
            viewport_ptr.append(self.add::<f32>(viewport_rect));
            let depth_range = &[viewport.depth.start as f64, viewport.depth.end as f64];
            depth_range_ptr.append(self.add::<f64>(depth_range));
            len += 1;
        }

        match len {
            0 => {
                error!("Number of viewports can not be zero.");
                self.cache.error_state = true;
            }
            n if n + first_viewport as usize <= self.limits.max_viewports => {
                self.push_cmd(Command::SetViewports {
                    first_viewport,
                    viewport_ptr,
                    depth_range_ptr,
                });
            }
            _ => {
                error!("Number of viewports and first viewport index exceed the number of maximum viewports");
                self.cache.error_state = true;
            }
        }
    }

    unsafe fn set_scissors<T>(&mut self, first_scissor: u32, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        let mut scissors_ptr = BufferSlice { offset: 0, size: 0 };
        let mut len = 0;
        for scissor in scissors {
            let scissor = scissor.borrow();
            let scissor = &[
                scissor.x as i32,
                scissor.y as i32,
                scissor.w as i32,
                scissor.h as i32,
            ];
            scissors_ptr.append(self.add::<i32>(scissor));
            len += 1;
        }

        match len {
            0 => {
                error!("Number of scissors can not be zero.");
                self.cache.error_state = true;
            }
            n if n + first_scissor as usize <= self.limits.max_viewports => {
                self.push_cmd(Command::SetScissors(first_scissor, scissors_ptr));
            }
            _ => {
                error!("Number of scissors and first scissor index exceed the maximum number of viewports");
                self.cache.error_state = true;
            }
        }
    }

    unsafe fn set_stencil_reference(&mut self, faces: pso::Face, value: pso::StencilValue) {
        assert!(!faces.is_empty());

        let mut front = 0;
        let mut back = 0;

        if let Some((last_front, last_back)) = self.cache.stencil_ref {
            front = last_front;
            back = last_back;
        }

        if faces.contains(pso::Face::FRONT) {
            front = value;
        }

        if faces.contains(pso::Face::BACK) {
            back = value;
        }

        // Only cache the stencil references values until
        // we assembled all the pieces to set the stencil state
        // from the pipeline.
        self.cache.stencil_ref = Some((front, back));
    }

    unsafe fn set_stencil_read_mask(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        unimplemented!();
    }

    unsafe fn set_stencil_write_mask(&mut self, _faces: pso::Face, _value: pso::StencilValue) {
        unimplemented!();
    }

    unsafe fn set_blend_constants(&mut self, cv: pso::ColorValue) {
        if self.cache.blend_color != Some(cv) {
            self.cache.blend_color = Some(cv);
            self.push_cmd(Command::SetBlendColor(cv));
        }
    }

    unsafe fn set_depth_bounds(&mut self, _: Range<f32>) {
        warn!("Depth bounds test is not supported");
    }

    unsafe fn set_line_width(&mut self, _width: f32) {
        unimplemented!()
    }

    unsafe fn set_depth_bias(&mut self, _depth_bias: pso::DepthBias) {
        unimplemented!()
    }

    unsafe fn bind_graphics_pipeline(&mut self, pipeline: &n::GraphicsPipeline) {
        let n::GraphicsPipeline {
            primitive,
            patch_size,
            program,
            ref blend_targets,
            ref attributes,
            ref vertex_buffers,
            ref uniforms,
            rasterizer,
            depth,
        } = *pipeline;

        if self.cache.primitive != Some(primitive) {
            self.cache.primitive = Some(primitive);
        }

        if self.cache.patch_size != patch_size {
            self.cache.patch_size = patch_size;
            if let Some(size) = patch_size {
                self.push_cmd(Command::SetPatchSize(size));
            }
        }

        if self.cache.program != Some(program) {
            self.cache.program = Some(program);
            self.push_cmd(Command::BindProgram(program));
        }

        self.cache.attributes = attributes.clone();

        self.cache.vertex_buffer_descs = vertex_buffers.clone();

        self.cache.uniforms = uniforms.clone();

        self.update_blend_targets(blend_targets);

        self.push_cmd(Command::BindRasterizer { 
            rasterizer, 
        });
        self.push_cmd(Command::BindDepth { 
            depth,
        });
    }

    unsafe fn bind_graphics_descriptor_sets<I, J>(
        &mut self,
        layout: &n::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<n::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<command::DescriptorSetOffset>,
    {
        assert!(offsets.into_iter().next().is_none()); // TODO: offsets unsupported

        let mut set = first_set as _;
        let drd = &*layout.desc_remap_data.read().unwrap();

        for desc_set in sets {
            let desc_set = desc_set.borrow();
            let bindings = desc_set.bindings.lock().unwrap();
            for new_binding in &*bindings {
                match new_binding {
                    n::DescSetBindings::Buffer {
                        ty: btype,
                        binding,
                        buffer,
                        offset,
                        size,
                    } => {
                        let btype = match btype {
                            n::BindingTypes::UniformBuffers => glow::UNIFORM_BUFFER,
                            n::BindingTypes::Images => panic!("Wrong desc set binding"),
                        };
                        for binding in drd
                            .get_binding(n::BindingTypes::UniformBuffers, set, *binding)
                            .unwrap()
                        {
                            self.push_cmd(Command::BindBufferRange(
                                btype,
                                *binding,
                                *buffer,
                                *offset as i32,
                                *size as i32,
                            ))
                        }
                    }
                    n::DescSetBindings::Texture(binding, texture) => {
                        for binding in drd
                            .get_binding(n::BindingTypes::Images, set, *binding)
                            .unwrap()
                        {
                            self.push_cmd(Command::BindTexture(*binding, *texture))
                        }
                    }
                    n::DescSetBindings::Sampler(binding, sampler) => {
                        for binding in drd
                            .get_binding(n::BindingTypes::Images, set, *binding)
                            .unwrap()
                        {
                            self.push_cmd(Command::BindSampler(*binding, *sampler))
                        }
                    }
                    n::DescSetBindings::SamplerInfo(binding, sinfo) => {
                        let mut all_txts = drd
                            .get_binding(n::BindingTypes::Images, set, *binding)
                            .unwrap()
                            .into_iter()
                            .flat_map(|binding| {
                                bindings.iter().filter_map(move |b| {
                                    if let n::DescSetBindings::Texture(b, t) = b {
                                        let nbs =
                                            drd.get_binding(n::BindingTypes::Images, set, *b)?;
                                        if nbs.contains(binding) {
                                            Some((*binding, *t))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                            })
                            .collect::<Vec<_>>();

                        // TODO: Check that other samplers aren't using the same
                        // textures as in `all_txts` unless all the bindings of that
                        // texture are gonna be unbound or the two samplers have
                        // identical properties.

                        all_txts.sort_unstable_by(|a, b| a.1.cmp(&b.1));
                        all_txts.dedup_by(|a, b| a.1 == b.1);

                        for (binding, txt) in all_txts {
                            self.push_cmd(Command::SetTextureSamplerSettings(
                                binding,
                                txt,
                                sinfo.clone(),
                            ))
                        }
                    }
                }
            }
            set += 1;
        }
    }

    unsafe fn bind_compute_pipeline(&mut self, pipeline: &n::ComputePipeline) {
        let n::ComputePipeline { program } = *pipeline;

        if self.cache.program != Some(program) {
            self.cache.program = Some(program);
            self.push_cmd(Command::BindProgram(program));
        }
    }

    unsafe fn bind_compute_descriptor_sets<I, J>(
        &mut self,
        _layout: &n::PipelineLayout,
        _first_set: usize,
        _sets: I,
        _offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<n::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<command::DescriptorSetOffset>,
    {
        // TODO
    }

    unsafe fn dispatch(&mut self, count: hal::WorkGroupCount) {
        self.push_cmd(Command::Dispatch(count));
    }

    unsafe fn dispatch_indirect(&mut self, buffer: &n::Buffer, offset: buffer::Offset) {
        self.push_cmd(Command::DispatchIndirect(buffer.raw, offset));
    }

    unsafe fn copy_buffer<T>(&mut self, src: &n::Buffer, dst: &n::Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<command::BufferCopy>,
    {
        let old_offset = self.buf.offset;

        for region in regions {
            let r = region.borrow().clone();
            let cmd = Command::CopyBufferToBuffer(src.raw, dst.raw, r);
            self.push_cmd(cmd);
        }

        if self.buf.offset == old_offset {
            error!("At least one region must be specified");
        }
    }

    unsafe fn copy_image<T>(
        &mut self,
        src: &n::Image,
        _src_layout: image::Layout,
        dst: &n::Image,
        _dst_layout: image::Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::ImageCopy>,
    {
        let old_offset = self.buf.offset;

        for region in regions {
            let r = region.borrow().clone();
            let cmd = match dst.kind {
                n::ImageKind::Surface(s) => Command::CopyImageToSurface(src.kind, s, r),
                n::ImageKind::Texture(t) => Command::CopyImageToTexture(src.kind, t, r),
            };
            self.push_cmd(cmd);
        }

        if self.buf.offset == old_offset {
            error!("At least one region must be specified");
        }
    }

    unsafe fn copy_buffer_to_image<T>(
        &mut self,
        src: &n::Buffer,
        dst: &n::Image,
        _: image::Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        let old_size = self.buf.size;

        for region in regions {
            let r = region.borrow().clone();
            let cmd = match dst.kind {
                n::ImageKind::Surface(s) => Command::CopyBufferToSurface(src.raw, s, r),
                n::ImageKind::Texture(t) => Command::CopyBufferToTexture(src.raw, t, r),
            };
            self.push_cmd(cmd);
        }

        if self.buf.size == old_size {
            error!("At least one region must be specified");
        }
    }

    unsafe fn copy_image_to_buffer<T>(
        &mut self,
        src: &n::Image,
        _: image::Layout,
        dst: &n::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<command::BufferImageCopy>,
    {
        let old_size = self.buf.size;

        for region in regions {
            let r = region.borrow().clone();
            let cmd = match src.kind {
                n::ImageKind::Surface(s) => Command::CopySurfaceToBuffer(s, dst.raw, r),
                n::ImageKind::Texture(t) => Command::CopyTextureToBuffer(t, dst.raw, r),
            };
            self.push_cmd(cmd);
        }

        if self.buf.size == old_size {
            error!("At least one region must be specified");
        }
    }

    unsafe fn draw(
        &mut self,
        vertices: Range<hal::VertexCount>,
        instances: Range<hal::InstanceCount>,
    ) {
        self.bind_attributes();

        match self.cache.primitive {
            Some(primitive) => {
                self.push_cmd(Command::Draw {
                    primitive,
                    vertices,
                    instances,
                });
            }
            None => {
                warn!("No primitive bound. An active pipeline needs to be bound before calling `draw`.");
                self.cache.error_state = true;
            }
        }
    }

    unsafe fn draw_indexed(
        &mut self,
        indices: Range<hal::IndexCount>,
        base_vertex: hal::VertexOffset,
        instances: Range<hal::InstanceCount>,
    ) {
        self.bind_attributes();

        let (start, index_type) = match self.cache.index_type {
            Some(hal::IndexType::U16) => (indices.start * 2, glow::UNSIGNED_SHORT),
            Some(hal::IndexType::U32) => (indices.start * 4, glow::UNSIGNED_INT),
            None => {
                warn!("No index type bound. An index buffer needs to be bound before calling `draw_indexed`.");
                self.cache.error_state = true;
                return;
            }
        };
        match self.cache.primitive {
            Some(primitive) => {
                self.push_cmd(Command::DrawIndexed {
                    primitive,
                    index_type,
                    index_count: indices.end - indices.start,
                    index_buffer_offset: start as _,
                    base_vertex,
                    instances,
                });
            }
            None => {
                warn!("No primitive bound. An active pipeline needs to be bound before calling `draw_indexed`.");
                self.cache.error_state = true;
            }
        }
    }

    unsafe fn draw_indirect(
        &mut self,
        _buffer: &n::Buffer,
        _offset: buffer::Offset,
        _draw_count: hal::DrawCount,
        _stride: u32,
    ) {
        unimplemented!()
    }

    unsafe fn draw_indexed_indirect(
        &mut self,
        _buffer: &n::Buffer,
        _offset: buffer::Offset,
        _draw_count: hal::DrawCount,
        _stride: u32,
    ) {
        unimplemented!()
    }

    unsafe fn begin_query(&mut self, _query: query::Query<Backend>, _flags: query::ControlFlags) {
        unimplemented!()
    }

    unsafe fn copy_query_pool_results(
        &mut self,
        _pool: &(),
        _queries: Range<query::Id>,
        _buffer: &n::Buffer,
        _offset: buffer::Offset,
        _stride: buffer::Offset,
        _flags: query::ResultFlags,
    ) {
        unimplemented!()
    }

    unsafe fn end_query(&mut self, _query: query::Query<Backend>) {
        unimplemented!()
    }

    unsafe fn reset_query_pool(&mut self, _pool: &(), _queries: Range<query::Id>) {
        unimplemented!()
    }

    unsafe fn write_timestamp(&mut self, _: pso::PipelineStage, _: query::Query<Backend>) {
        unimplemented!()
    }

    unsafe fn push_graphics_constants(
        &mut self,
        _layout: &n::PipelineLayout,
        _stages: pso::ShaderStageFlags,
        offset: u32,
        constants: &[u32],
    ) {
        let buffer = self.add(constants);

        let uniforms = &self.cache.uniforms;
        if uniforms.is_empty() {
            unimplemented!()
        }

        let uniform = if offset == 0 {
            // If offset is zero, we can just return the first item
            // in our uniform list
            uniforms.get(0).unwrap()
        } else {
            match uniforms.binary_search_by(|uniform| uniform.offset.cmp(&offset as _)) {
                Ok(index) => uniforms.get(index).unwrap(),
                Err(_) => panic!("No uniform found at offset: {}", offset),
            }
        }
        .clone();

        self.push_cmd(Command::BindUniform { uniform, buffer });
    }

    unsafe fn push_compute_constants(
        &mut self,
        _layout: &n::PipelineLayout,
        _offset: u32,
        _constants: &[u32],
    ) {
        unimplemented!()
    }

    unsafe fn execute_commands<'a, T, I>(&mut self, _buffers: I)
    where
        T: 'a + Borrow<RawCommandBuffer>,
        I: IntoIterator<Item = &'a T>,
    {
        unimplemented!()
    }
}

/// Avoids creating second mutable borrows of `self` by requiring mutable
/// references only to the fields it needs. Many functions will simply use
/// `push_cmd`, but this is needed when the caller would like to perform a
/// partial borrow to `self`. For example, iterating through a field on
/// `self` and calling `self.push_cmd` per iteration.
fn push_cmd_internal(
    id: &u64,
    memory: &mut Arc<Mutex<BufferMemory>>,
    buffer: &mut BufferSlice,
    cmd: Command,
) {
    let mut memory = memory
        .try_lock()
        .expect("Trying to record a command buffers, while memory is in-use.");

    let cmd_buffer = match *memory {
        BufferMemory::Linear(ref mut buffer) => &mut buffer.commands,
        BufferMemory::Individual {
            ref mut storage, ..
        } => &mut storage.get_mut(id).unwrap().commands,
    };

    cmd_buffer.push(cmd);

    buffer.append(BufferSlice {
        offset: cmd_buffer.len() as u32 - 1,
        size: 1,
    });
}
