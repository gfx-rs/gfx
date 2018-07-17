use {
    AutoreleasePool, Backend, PrivateDisabilities, Shared, validate_line_width,
    BufferPtr, TexturePtr, SamplerPtr,
};
use {conversions as conv, native, soft, window};
use internal::{BlitVertex, Channel, ClearKey, ClearVertex, ServicePipes};
use lock::Mutex;

use std::borrow::Borrow;
use std::cell::RefCell;
use std::ops::{Deref, Range};
use std::sync::Arc;
use std::{iter, mem, slice, time};

use hal::{buffer, command as com, error, memory, pool, pso};
use hal::{DrawCount, SwapImageIndex, VertexCount, VertexOffset, InstanceCount, IndexCount, WorkGroupCount};
use hal::backend::FastHashMap;
use hal::format::{Aspects, Format, FormatDesc};
use hal::image::{Extent, Filter, Layout, Level, SubresourceRange};
use hal::pass::{AttachmentLoadOp, AttachmentOps};
use hal::query::{Query, QueryControl, QueryId};
use hal::queue::{RawCommandQueue, RawSubmission};
use hal::range::RangeArg;

use foreign_types::{ForeignType, ForeignTypeRef};
use metal::{self, MTLViewport, MTLScissorRect, MTLPrimitiveType, MTLIndexType, MTLSize};
use cocoa::foundation::{NSUInteger, NSInteger, NSRange};
use block::{ConcreteBlock};
use smallvec::SmallVec;


const WORD_SIZE: usize = 4;
const WORD_ALIGNMENT: u64 = WORD_SIZE as _;
/// Enable an optimization to have multi-layered render passed
/// with clear operations set up to implement our `clear_image`
/// Note: currently doesn't work, needs a repro case for Apple
const CLEAR_IMAGE_ARRAY: bool = false;
/// Number of frames to average when reporting the performance counters.
const COUNTERS_REPORT_WINDOW: usize = 0;

pub struct QueueInner {
    raw: metal::CommandQueue,
    reserve: Range<usize>,
    debug_retain_references: bool,
}

#[must_use]
#[derive(Debug)]
pub struct Token {
    active: bool,
}

impl Drop for Token {
    fn drop(&mut self) {
        // poor man's linear type...
        debug_assert!(!self.active);
    }
}

impl QueueInner {
    pub(crate) fn new(
        device: &metal::DeviceRef,
        pool_size: Option<usize>,
    ) -> Self {
        match pool_size {
            Some(count) => QueueInner {
                raw: device.new_command_queue_with_max_command_buffer_count(count as u64),
                reserve: 0 .. count,
                debug_retain_references: false,
            },
            None => QueueInner {
                raw: device.new_command_queue(),
                reserve: 0 .. 64,
                debug_retain_references: true,
            },
        }
    }

    /// Spawns a command buffer from a virtual pool.
    pub(crate) fn spawn(&mut self) -> (metal::CommandBuffer, Token) {
        let _pool = AutoreleasePool::new();
        self.reserve.start += 1;
        let cmd_buf = self.spawn_temp().to_owned();
        (cmd_buf, Token { active: true })
    }

    fn spawn_temp(&self) -> &metal::CommandBufferRef {
        if self.debug_retain_references {
            self.raw.new_command_buffer()
        } else {
            self.raw.new_command_buffer_with_unretained_references()
        }
    }

    /// Returns a command buffer to a virtual pool.
    pub(crate) fn release(&mut self, mut token: Token) {
        token.active = false;
        self.reserve.start -= 1;
    }

    /// Block until GPU is idle.
    pub(crate) fn wait_idle(queue: &Mutex<Self>) {
        debug!("waiting for idle");
        let _pool = AutoreleasePool::new();
        // note: we deliberately don't hold the Mutex lock while waiting,
        // since the completion handlers need to access it.
        let (cmd_buf, token) = queue.lock().spawn();
        cmd_buf.set_label("empty");
        cmd_buf.commit();
        cmd_buf.wait_until_completed();
        queue.lock().release(token);
    }
}

type CommandBufferInnerPtr = Arc<RefCell<CommandBufferInner>>;

pub struct CommandPool {
    pub(crate) shared: Arc<Shared>,
    pub(crate) allocated: Vec<CommandBufferInnerPtr>,
}

unsafe impl Send for CommandPool {}
unsafe impl Sync for CommandPool {}

#[derive(Clone)]
pub struct CommandBuffer {
    inner: CommandBufferInnerPtr,
    shared: Arc<Shared>,
    state: State,
    temp: Temp,
}

unsafe impl Send for CommandBuffer {}
unsafe impl Sync for CommandBuffer {}

#[derive(Clone)]
struct Temp {
    clear_vertices: Vec<ClearVertex>,
    blit_vertices: FastHashMap<(Aspects, Level), Vec<BlitVertex>>,
}

#[derive(Clone)]
struct RenderPipelineState {
    raw: metal::RenderPipelineState,
    ds_desc: pso::DepthStencilDesc,
    vbuf_map: native::VertexBufferMap,
    at_formats: SmallVec<[Option<Format>; 8]>,
}

#[derive(Clone)]
struct State {
    viewport: Option<MTLViewport>,
    scissors: Option<MTLScissorRect>,
    blend_color: Option<pso::ColorValue>,
    render_pso: Option<RenderPipelineState>,
    /// A flag to handle edge cases of Vulkan binding inheritance:
    /// we don't want to consider the current PSO bound for a new pass if it's not compatible.
    render_pso_is_compatible: bool,
    compute_pso: Option<metal::ComputePipelineState>,
    work_group_size: MTLSize,
    primitive_type: MTLPrimitiveType,
    resources_vs: StageResources,
    resources_fs: StageResources,
    resources_cs: StageResources,
    index_buffer: Option<IndexBuffer<BufferPtr>>,
    rasterizer_state: Option<native::RasterizerState>,
    depth_bias: pso::DepthBias,
    stencil: native::StencilState<pso::StencilValue>,
    push_constants: Vec<u32>,
    vertex_buffers: Vec<Option<(BufferPtr, u64)>>,
    framebuffer_inner: native::FramebufferInner,
}

impl State {
    fn reset_resources(&mut self) {
        self.resources_vs.clear();
        self.resources_fs.clear();
        self.resources_cs.clear();
        self.push_constants.clear();
        self.vertex_buffers.clear();
    }

    fn clamp_scissor(sr: MTLScissorRect, extent: Extent) -> MTLScissorRect {
        // sometimes there is not even an active render pass at this point
        let x = sr.x.min(extent.width.max(1) as u64 - 1);
        let y = sr.y.min(extent.height.max(1) as u64 - 1);
        //TODO: handle the zero scissor size sensibly
        MTLScissorRect {
            x,
            y,
            width: ((sr.x + sr.width).min(extent.width as u64) - x).max(1),
            height: ((sr.y + sr.height).min(extent.height as u64) - y).max(1),
        }
    }

    fn make_render_commands<'a>(
        &'a self, aspects: Aspects
    ) -> impl Iterator<Item = soft::RenderCommand<&'a soft::Own>> {
        // Apply previously bound values for this command buffer
        let com_vp = self.viewport.map(soft::RenderCommand::SetViewport);
        let com_scissor = self.scissors.map(|sr| soft::RenderCommand::SetScissor(
            Self::clamp_scissor(sr, self.framebuffer_inner.extent)
        ));
        let com_blend = if aspects.contains(Aspects::COLOR) {
            self.blend_color.map(soft::RenderCommand::SetBlendColor)
        } else {
            None
        };
        let com_depth_bias = if aspects.contains(Aspects::DEPTH) {
            Some(soft::RenderCommand::SetDepthBias(self.depth_bias))
        } else {
            None
        };
        let com_pso = if self.render_pso_is_compatible {
            let rast = self.rasterizer_state.clone();
            self.render_pso.as_ref().map(|ps| {
                soft::RenderCommand::BindPipeline(&*ps.raw, rast)
            })
        } else {
            None
        };

        let render_resources = iter::once(&self.resources_vs).chain(iter::once(&self.resources_fs));
        let push_constants = self.push_constants.as_slice();
        let com_resources = [pso::Stage::Vertex, pso::Stage::Fragment]
            .iter()
            .zip(render_resources)
            .flat_map(move |(&stage, resources)| {
                let com_buffers = resources.buffers.iter().enumerate().filter_map(move |(i, resource)| {
                    resource.map(|(buffer, offset)| {
                        soft::RenderCommand::BindBuffer {
                            stage,
                            index: i as _,
                            buffer: Some(buffer),
                            offset,
                        }
                    })
                });
                let com_textures = resources.textures.iter().enumerate().filter_map(move |(i, resource)| {
                    resource.map(|texture| {
                        soft::RenderCommand::BindTexture {
                            stage,
                            index: i as _,
                            texture: Some(texture),
                        }
                    })
                });
                let com_samplers = resources.samplers.iter().enumerate().filter_map(move |(i, resource)| {
                    resource.map(|sampler| {
                        soft::RenderCommand::BindSampler {
                            stage,
                            index: i as _,
                            sampler: Some(sampler),
                        }
                    })
                });
                let com_push_constants = resources.push_constants_buffer_id
                    .map(|id| soft::RenderCommand::BindBufferData {
                        stage,
                        index: id  as _,
                        words: push_constants,
                    });
                com_buffers
                    .chain(com_textures)
                    .chain(com_samplers)
                    .chain(com_push_constants)
            });

        com_vp
            .into_iter()
            .chain(com_scissor)
            .chain(com_blend)
            .chain(com_depth_bias)
            .chain(com_pso)
            //.chain(com_ds) // done outside
            .chain(com_resources)
    }

    fn make_compute_commands<'a>(&'a self) -> impl Iterator<Item = soft::ComputeCommand<&'a soft::Own>> {
        let com_pso = self.compute_pso
            .as_ref()
            .map(|pso| soft::ComputeCommand::BindPipeline(&**pso));
        let com_buffers = self.resources_cs.buffers
            .iter()
            .enumerate()
            .filter_map(|(i, resource)| {
                resource.map(|(buffer, offset)| {
                    soft::ComputeCommand::BindBuffer {
                        index: i as _,
                        buffer: Some(buffer),
                        offset,
                    }
                })
            });
        let com_textures = self.resources_cs.textures
            .iter()
            .enumerate()
            .filter_map(|(i, ref resource)| {
                resource.map(|texture| {
                    soft::ComputeCommand::BindTexture {
                        index: i as _,
                        texture: Some(texture),
                    }
                })
            });
        let com_samplers = self.resources_cs.samplers
            .iter()
            .enumerate()
            .filter_map(|(i, ref resource)| {
                resource.map(|sampler| {
                    soft::ComputeCommand::BindSampler {
                        index: i as _,
                        sampler: Some(sampler),
                    }
                })
            });
        let com_push_constants = self.resources_cs.push_constants_buffer_id
            .map(|id| soft::ComputeCommand::BindBufferData {
                index: id as _,
                words: self.push_constants.as_slice(),
            });

        com_pso
            .into_iter()
            .chain(com_buffers)
            .chain(com_textures)
            .chain(com_samplers)
            .chain(com_push_constants)
    }

    fn set_vertex_buffers(&mut self) -> u64 {
        let map = match self.render_pso {
            Some(ref ps) => &ps.vbuf_map,
            None => return 0
        };

        let vs_buffers = &mut self.resources_vs.buffers;
        let mut mask = 0;
        for (&(binding, extra_offset), vb) in map {
            let index = vb.binding as usize;
            while vs_buffers.len() <= index {
                vs_buffers.push(None)
            }
            let (buffer, offset) = match self.vertex_buffers.get(binding as usize) {
                Some(&Some((ref buffer, base_offset))) => (buffer, extra_offset as u64 + base_offset),
                // being unable to bind a buffer here is technically fine, since before this moment
                // and actual rendering there might be more bind calls
                _ => continue,
            };

            if let Some((ref old_buffer, old_offset)) = vs_buffers[index] {
                if old_buffer.as_ptr() == buffer.as_ptr() && old_offset == offset {
                    continue; // already bound
                }
            }
            vs_buffers[index] = Some((buffer.clone(), offset));
            mask |= 1<<index;
        }
        mask
    }

    fn iter_vertex_buffers<'a>(&'a self, mask: u64) -> impl Iterator<Item = soft::RenderCommand<&'a soft::Own>> {
        self.resources_vs.buffers
            .iter()
            .enumerate()
            .filter_map(move |(index, maybe_buffer)| {
                if mask & (1u64 << index) != 0 {
                    maybe_buffer.map(|(buffer, offset)| {
                        soft::RenderCommand::BindBuffer {
                            stage: pso::Stage::Vertex,
                            index,
                            buffer: Some(buffer),
                            offset,
                        }
                    })
                } else {
                    None
                }
            })
    }

    fn sync_depth_stencil<'a>(
        &'a self, pipes: &'a mut ServicePipes, device: &Mutex<metal::Device>,
    ) -> Option<soft::RenderCommand<&'a soft::Own>> {
        let mut desc = match self.render_pso {
            Some(ref ps) => ps.ds_desc.clone(),
            None => return None,
        };

        if !self.framebuffer_inner.aspects.contains(Aspects::DEPTH) {
            desc.depth = pso::DepthTest::Off;
        }
        if !self.framebuffer_inner.aspects.contains(Aspects::STENCIL) {
            desc.stencil = pso::StencilTest::Off;
        }

        if let pso::StencilTest::On { ref mut front, ref mut back } = desc.stencil {
            front.reference = pso::State::Dynamic;
            if front.mask_read.is_dynamic() {
                front.mask_read = pso::State::Static(self.stencil.front_read_mask);
            }
            if front.mask_write.is_dynamic() {
                front.mask_write = pso::State::Static(self.stencil.front_write_mask);
            }
            back.reference = pso::State::Dynamic;
            if back.mask_read.is_dynamic() {
                back.mask_read = pso::State::Static(self.stencil.back_read_mask);
            }
            if back.mask_write.is_dynamic() {
                back.mask_write = pso::State::Static(self.stencil.back_write_mask);
            }
        }

        let state = pipes.depth_stencil_states.get(desc, device);
        Some(soft::RenderCommand::SetDepthStencilState(state))
    }

    fn set_depth_bias<'a>(&mut self, depth_bias: &pso::DepthBias) -> soft::RenderCommand<&'a soft::Own> {
        self.depth_bias = *depth_bias;
        soft::RenderCommand::SetDepthBias(*depth_bias)
    }

    fn push_vs_constants<'a>(&'a mut self, id: u32) -> soft::RenderCommand<&'a soft::Own>{
        self.resources_vs.push_constants_buffer_id = Some(id);
        soft::RenderCommand::BindBufferData {
            stage: pso::Stage::Vertex,
            index: id as usize,
            words: &self.push_constants,
        }
    }

    fn push_ps_constants<'a>(&'a mut self, id: u32) -> soft::RenderCommand<&'a soft::Own> {
        self.resources_fs.push_constants_buffer_id = Some(id);
        soft::RenderCommand::BindBufferData {
            stage: pso::Stage::Fragment,
            index: id as usize,
            words: &self.push_constants,
        }
    }

    fn push_cs_constants<'a>(&'a mut self, id: u32) -> soft::ComputeCommand<&'a soft::Own> {
        self.resources_cs.push_constants_buffer_id = Some(id);
        soft::ComputeCommand::BindBufferData {
            index: id as usize,
            words: &self.push_constants,
        }
    }

    fn set_viewport<'a>(
        &mut self, vp: &'a pso::Viewport, disabilities: &PrivateDisabilities
    ) -> soft::RenderCommand<&'a soft::Own> {
        let viewport = MTLViewport {
            originX: vp.rect.x as _,
            originY: vp.rect.y as _,
            width: vp.rect.w as _,
            height: vp.rect.h as _,
            znear: vp.depth.start as _,
            zfar: if disabilities.broken_viewport_near_depth {
                (vp.depth.end - vp.depth.start) as _
            } else {
                vp.depth.end as _
            },
        };
        self.viewport = Some(viewport);
        soft::RenderCommand::SetViewport(viewport)
    }

    fn set_scissor<'a>(&mut self, rect: &'a pso::Rect) -> soft::RenderCommand<&'a soft::Own> {
        let scissor = MTLScissorRect {
            x: rect.x as _,
            y: rect.y as _,
            width: rect.w as _,
            height: rect.h as _,
        };
        self.scissors = Some(scissor);
        let clamped = State::clamp_scissor(scissor, self.framebuffer_inner.extent);
        soft::RenderCommand::SetScissor(clamped)
    }

    fn set_blend_color<'a>(&mut self, color: &'a pso::ColorValue) -> soft::RenderCommand<&'a soft::Own> {
        self.blend_color = Some(*color);
        soft::RenderCommand::SetBlendColor(*color)
    }

    fn update_push_constants(
        &mut self,
        offset: u32,
        constants: &[u32],
    ) {
        assert_eq!(offset % WORD_ALIGNMENT as u32, 0);
        let offset = (offset  / WORD_ALIGNMENT as u32) as usize;
        let data = &mut self.push_constants;
        while data.len() < offset + constants.len() {
            data.push(0);
        }
        data[offset .. offset + constants.len()].copy_from_slice(constants);
    }
}

#[derive(Clone, Debug)]
struct StageResources {
    buffers: Vec<Option<(BufferPtr, buffer::Offset)>>,
    textures: Vec<Option<TexturePtr>>,
    samplers: Vec<Option<SamplerPtr>>,
    push_constants_buffer_id: Option<u32>,
}

impl StageResources {
    fn new() -> Self {
        StageResources {
            buffers: Vec::new(),
            textures: Vec::new(),
            samplers: Vec::new(),
            push_constants_buffer_id: None,
        }
    }

    fn clear(&mut self) {
        self.buffers.clear();
        self.textures.clear();
        self.samplers.clear();
        self.push_constants_buffer_id = None;
    }

    fn set_buffer(&mut self, slot: usize, buffer: BufferPtr, offset: buffer::Offset) -> bool {
        if self.buffers.len() <= slot {
            self.buffers.resize(slot + 1, None);
        }
        let value = Some((buffer, offset));
        if self.buffers[slot] != value {
            self.buffers[slot] = value;
            true
        } else {
            false
        }
    }

    fn set_textures<F>(
        &mut self, start: usize, textures: &[Option<(TexturePtr, Layout)>], mut update: F
    ) where
        F: FnMut(usize, Option<TexturePtr>)
    {
        if self.textures.len() < start + textures.len() {
            self.textures.resize(start + textures.len(), None);
        }
        for (i, (out, maybe)) in self
            .textures[start..]
            .iter_mut()
            .zip(textures)
            .enumerate()
        {
            let value = maybe.map(|(t, _)| t);
            if *out != value {
                *out = value;
                update(start + i, value);
            }
        }
    }

    fn set_samplers<F>(
        &mut self, start: usize, samplers: &[Option<SamplerPtr>], mut update: F
    ) where
        F: FnMut(usize, Option<SamplerPtr>)
    {
        if self.samplers.len() < start + samplers.len() {
            self.samplers.resize(start + samplers.len(), None);
        }
        for (i, (out, &value)) in self
            .samplers[start..]
            .iter_mut()
            .zip(samplers)
            .enumerate()
        {
            if *out != value {
                *out = value;
                update(start + i, value);
            }
        }
    }
}

#[derive(Debug)]
enum CommandSink {
    Immediate {
        cmd_buffer: metal::CommandBuffer,
        token: Token,
        encoder_state: EncoderState,
    },
    Deferred {
        passes: Vec<soft::Pass>,
        is_encoding: bool,
    },
}

enum PassDoor<'a> {
    Open,
    Closed { label: &'a str },
}

/// A helper temporary object that consumes state-setting commands only
/// applicable to a render pass currently encoded.
enum PreRender<'a> {
    Immediate(&'a metal::RenderCommandEncoder),
    Deferred(&'a mut Vec<soft::RenderCommand<soft::Own>>),
    Void,
}

impl<'a> PreRender<'a> {
    fn is_void(&self) -> bool {
        match *self {
            PreRender::Void => true,
            _ => false,
        }
    }

    fn issue<'b>(&mut self, command: soft::RenderCommand<&'b soft::Own>) {
        match *self {
            PreRender::Immediate(encoder) => exec_render(encoder, command),
            PreRender::Deferred(ref mut list) => list.push(command.own()),
            PreRender::Void => (),
        }
    }
}

/// A helper temporary object that consumes state-setting commands only
/// applicable to a compute pass currently encoded.
enum PreCompute<'a> {
    Immediate(&'a metal::ComputeCommandEncoder),
    Deferred(&'a mut Vec<soft::ComputeCommand<soft::Own>>),
    Void,
}

impl<'a> PreCompute<'a> {
    fn issue<'b>(&mut self, command: soft::ComputeCommand<&'b soft::Own>) {
        match *self {
            PreCompute::Immediate(encoder) => exec_compute(encoder, command),
            PreCompute::Deferred(ref mut list) => list.push(command.own()),
            PreCompute::Void => (),
        }
    }
}

impl CommandSink {
    /// Start issuing pre-render commands. Those can be rejected, so the caller is responsible
    /// for updating the state cache accordingly, so that it's set upon the start of a next pass.
    fn pre_render(&mut self) -> PreRender {
        match *self {
            CommandSink::Immediate { encoder_state: EncoderState::Render(ref encoder), .. } => {
                PreRender::Immediate(encoder)
            }
            CommandSink::Deferred { ref mut passes, is_encoding: true } => {
                match passes.last_mut() {
                    Some(&mut soft::Pass::Render { commands: ref mut list, .. }) => PreRender::Deferred(list),
                    _ => PreRender::Void,
                }
            }
            _ => PreRender::Void
        }
    }

    /// Issue provided render commands, expecting that we are encoding a render pass.
    fn render_commands<'a, I>(&mut self, commands: I)
    where
        I: Iterator<Item = soft::RenderCommand<&'a soft::Own>>,
    {
        match *self {
            CommandSink::Immediate { ref mut encoder_state, .. } => {
                match *encoder_state {
                    EncoderState::Render(ref encoder) => {
                        for command in commands {
                            exec_render(encoder, command);
                        }
                    }
                    _ => panic!("Expected to be in render encoding state!")
                }
            }
            CommandSink::Deferred { ref mut passes, is_encoding } => {
                assert!(is_encoding);
                match passes.last_mut() {
                    Some(&mut soft::Pass::Render { commands: ref mut list, .. }) => {
                        list.extend(commands.into_iter().map(soft::RenderCommand::own));
                    }
                    _ => panic!("Active pass is not a render pass")
                }
            }
        }
    }

    /// Issue provided blit commands. This function doesn't expect an active blit pass,
    /// it will automatically start one when needed.
    fn blit_commands<'a, I>(&mut self, commands: I)
    where
        I: Iterator<Item = soft::BlitCommand<&'a soft::Own>>,
    {
        match *self {
            CommandSink::Immediate { ref cmd_buffer, ref mut encoder_state, .. } => {
                let current = match mem::replace(encoder_state, EncoderState::None) {
                    EncoderState::None => None,
                    EncoderState::Render(enc) => {
                        enc.end_encoding();
                        None
                    },
                    EncoderState::Blit(enc) => Some(enc),
                    EncoderState::Compute(enc) => {
                        enc.end_encoding();
                        None
                    },
                };
                let encoder = current.unwrap_or_else(|| {
                    cmd_buffer.new_blit_command_encoder().to_owned()
                });

                for command in commands {
                    exec_blit(&encoder, command);
                }

                *encoder_state = EncoderState::Blit(encoder);
            }
            CommandSink::Deferred { ref mut passes, .. } => {
                let owned_commands = commands.into_iter().map(soft::BlitCommand::own);
                if let Some(&mut soft::Pass::Blit(ref mut list)) = passes.last_mut() {
                    list.extend(owned_commands);
                    return;
                }
                passes.push(soft::Pass::Blit(owned_commands.collect()));
            }
        }
    }

    /// Start issuing pre-compute commands. Those can be rejected, so the caller is responsible
    /// for updating the state cache accordingly, so that it's set upon the start of a next pass.
    fn pre_compute(&mut self) -> PreCompute {
        match *self {
            CommandSink::Immediate { encoder_state: EncoderState::Compute(ref encoder), .. } => {
                PreCompute::Immediate(encoder)
            }
            CommandSink::Deferred { ref mut passes, is_encoding: true } => {
                match passes.last_mut() {
                    Some(&mut soft::Pass::Compute(ref mut list)) => PreCompute::Deferred(list),
                    _ => PreCompute::Void,
                }
            }
            _ => PreCompute::Void
        }
    }

    /// Issue provided compute commands, expecting that we are encoding a compute pass.
    fn compute_commands<'a, I>(&mut self, commands: I)
    where
        I: Iterator<Item = soft::ComputeCommand<&'a soft::Own>>,
    {
        match *self {
            CommandSink::Immediate { ref mut encoder_state, .. } => {
                match *encoder_state {
                    EncoderState::Compute(ref encoder) => {
                        for command in commands {
                            exec_compute(encoder, command);
                        }
                    }
                    _ => panic!("Expected to be in compute pass"),
                }
            }
            CommandSink::Deferred { ref mut passes, .. } => {
                let owned_commands = commands.into_iter().map(soft::ComputeCommand::own);
                if let Some(&mut soft::Pass::Compute(ref mut list)) = passes.last_mut() {
                    list.extend(owned_commands);
                    return;
                }
                passes.push(soft::Pass::Compute(owned_commands.collect()));
            }
        }
    }

    fn stop_encoding(&mut self) {
        match *self {
            CommandSink::Immediate { ref mut encoder_state, .. } => {
                encoder_state.end();
            }
            CommandSink::Deferred { ref mut is_encoding, .. } => {
                *is_encoding = false;
            }
        }
    }

    fn begin_render_pass<'a, I>(
        &mut self,
        door: PassDoor,
        descriptor: &'a metal::RenderPassDescriptorRef,
        init_commands: I,
    ) where
        I: Iterator<Item = soft::RenderCommand<&'a soft::Own>>,
    {
        //assert!(AutoReleasePool::is_active());
        self.stop_encoding();

        match *self {
            CommandSink::Immediate { ref cmd_buffer, ref mut encoder_state, .. } => {
                let encoder = cmd_buffer.new_render_command_encoder(descriptor);
                for command in init_commands {
                    exec_render(encoder, command);
                }
                match door {
                    PassDoor::Open => {
                        *encoder_state = EncoderState::Render(encoder.to_owned())
                    }
                    PassDoor::Closed { label } => {
                        encoder.set_label(label);
                        encoder.end_encoding();
                    }
                }
            }
            CommandSink::Deferred { ref mut passes, ref mut is_encoding } => {
                *is_encoding = match door {
                    PassDoor::Open => true,
                    PassDoor::Closed {..} => false,
                };
                passes.push(soft::Pass::Render {
                    //Note: the original descriptor belongs to the framebuffer,
                    // and will me mutated afterwards.
                    desc: unsafe {
                        let desc: metal::RenderPassDescriptor = msg_send![descriptor, copy];
                        msg_send![desc.as_ptr(), retain];
                        desc
                    },
                    commands: init_commands.map(soft::RenderCommand::own).collect(),
                });
            }
        }
    }

    fn begin_compute_pass<'a, I>(
        &mut self,
        door: PassDoor,
        init_commands: I,
    ) where
        I: Iterator<Item = soft::ComputeCommand<&'a soft::Own>>,
    {
        self.stop_encoding();

        match *self {
            CommandSink::Immediate { ref cmd_buffer, ref mut encoder_state, .. } => {
                let _ap = AutoreleasePool::new();
                let encoder = cmd_buffer.new_compute_command_encoder();
                for command in init_commands {
                    exec_compute(encoder, command);
                }
                match door {
                    PassDoor::Open => {
                        *encoder_state = EncoderState::Compute(encoder.to_owned());
                    }
                    PassDoor::Closed { label } => {
                        encoder.set_label(label);
                        encoder.end_encoding();
                    }
                }
            }
            CommandSink::Deferred { ref mut passes, ref mut is_encoding } => {
                *is_encoding = match door {
                    PassDoor::Open => true,
                    PassDoor::Closed {..} => false,
                };
                passes.push(soft::Pass::Compute(
                    init_commands.map(soft::ComputeCommand::own).collect(),
                ));
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IndexBuffer<B> {
    buffer: B,
    offset: buffer::Offset,
    index_type: MTLIndexType,
}

pub struct CommandBufferInner {
    sink: Option<CommandSink>,
    retained_buffers: Vec<metal::Buffer>,
    retained_textures: Vec<metal::Texture>,
}

impl Drop for CommandBufferInner {
    fn drop(&mut self) {
        if self.sink.is_some() {
            error!("Command buffer not released properly!");
        }
    }
}

impl CommandBufferInner {
    pub(crate) fn reset(&mut self, shared: &Shared) {
        if let Some(CommandSink::Immediate { token, mut encoder_state, .. }) = self.sink.take() {
            encoder_state.end();
            shared.queue.lock().release(token);
        }
        self.retained_buffers.clear();
        self.retained_textures.clear();
    }

    fn sink(&mut self) -> &mut CommandSink {
        self.sink.as_mut().unwrap()
    }
}


#[derive(Debug)]
enum EncoderState {
    None,
    Blit(metal::BlitCommandEncoder),
    Render(metal::RenderCommandEncoder),
    Compute(metal::ComputeCommandEncoder),
}

impl EncoderState {
    fn end(&mut self) {
        match mem::replace(self, EncoderState::None)  {
            EncoderState::None => {}
            EncoderState::Render(ref encoder) => {
                encoder.end_encoding();
            }
            EncoderState::Blit(ref encoder) => {
                encoder.end_encoding();
            }
            EncoderState::Compute(ref encoder) => {
                encoder.end_encoding();
            }
        }
    }
}

fn div(a: u32, b: u32) -> u32 {
    assert_eq!(a % b, 0);
    a / b
}

fn compute_pitches(
    region: &com::BufferImageCopy, fd: &FormatDesc, extent: &MTLSize
) -> (u32, u32) {
    let buffer_width = if region.buffer_width == 0 {
        extent.width as u32
    } else {
        region.buffer_width
    };
    let buffer_height = if region.buffer_height == 0 {
        extent.height as u32
    } else {
        region.buffer_height
    };
    let row_pitch = div(buffer_width, fd.dim.0 as _) * (fd.bits / 8) as u32;
    let slice_pitch = div(buffer_height, fd.dim.1 as _) * row_pitch;
    (row_pitch, slice_pitch)
}

fn exec_render<'a>(encoder: &metal::RenderCommandEncoderRef, command: soft::RenderCommand<&'a soft::Own>) {
    use soft::RenderCommand as Cmd;
    match command {
        Cmd::SetViewport(viewport) => {
            encoder.set_viewport(viewport);
        }
        Cmd::SetScissor(scissor) => {
            encoder.set_scissor_rect(scissor);
        }
        Cmd::SetBlendColor(color) => {
            encoder.set_blend_color(color[0], color[1], color[2], color[3]);
        }
        Cmd::SetDepthBias(depth_bias) => {
            encoder.set_depth_bias(depth_bias.const_factor, depth_bias.slope_factor, depth_bias.clamp);
        }
        Cmd::SetDepthStencilState(depth_stencil) => {
            encoder.set_depth_stencil_state(depth_stencil);
        }
        Cmd::SetStencilReferenceValues(front, back) => {
            encoder.set_stencil_front_back_reference_value(front, back);
        }
        Cmd::BindBuffer { stage, index, buffer, offset } => {
            let native = buffer.as_ref().map(|b| b.as_native());
            match stage {
                pso::Stage::Vertex =>
                    encoder.set_vertex_buffer(index as _, offset as _, native),
                pso::Stage::Fragment =>
                    encoder.set_fragment_buffer(index as _, offset as _, native),
                _ => unimplemented!()
            }
        }
        Cmd::BindBufferData { stage, index, words } => {
            match stage {
                pso::Stage::Vertex =>
                    encoder.set_vertex_bytes(index as _, (words.len() * WORD_SIZE) as u64, words.as_ptr() as _),
                pso::Stage::Fragment =>
                    encoder.set_fragment_bytes(index as _, (words.len() * WORD_SIZE) as u64, words.as_ptr() as _),
                _ => unimplemented!()
            }
        }
        Cmd::BindTexture { stage, index, texture } => {
            let native = texture.as_ref().map(|t| t.as_native());
            match stage {
                pso::Stage::Vertex =>
                    encoder.set_vertex_texture(index as _, native),
                pso::Stage::Fragment =>
                    encoder.set_fragment_texture(index as _, native),
                _ => unimplemented!()
            }
        }
        Cmd::BindSampler { stage, index, sampler } => {
            let native = sampler.as_ref().map(|s| s.as_native());
            match stage {
                pso::Stage::Vertex =>
                    encoder.set_vertex_sampler_state(index as _, native),
                pso::Stage::Fragment =>
                    encoder.set_fragment_sampler_state(index as _, native),
                _ => unimplemented!()
            }
        }
        Cmd::BindPipeline(pipeline_state, rasterizer) => {
            encoder.set_render_pipeline_state(pipeline_state);
            if let Some(rs) = rasterizer {
                encoder.set_front_facing_winding(rs.front_winding);
                encoder.set_cull_mode(rs.cull_mode);
                encoder.set_depth_clip_mode(rs.depth_clip);
            }
        }
        Cmd::Draw { primitive_type, vertices, instances } =>  {
            /*if instances.start == 0 { //TODO: needs metal-rs breaking update
                encoder.draw_primitives_instanced(
                    primitive_type,
                    vertices.start as NSUInteger,
                    (vertices.end - vertices.start) as NSUInteger,
                    instances.end as NSUInteger,
                );
            } else*/ {
                encoder.draw_primitives_instanced_base_instance(
                    primitive_type,
                    vertices.start as NSUInteger,
                    (vertices.end - vertices.start) as NSUInteger,
                    (instances.end - instances.start) as NSUInteger,
                    instances.start as NSUInteger,
                );
            }
        }
        Cmd::DrawIndexed { primitive_type, index, indices, base_vertex, instances } => {
            let index_size = match index.index_type {
                MTLIndexType::UInt16 => 2,
                MTLIndexType::UInt32 => 4,
            };
            let index_offset = index.offset + indices.start as buffer::Offset * index_size;
            let index_buffer = index.buffer.as_native();
            // Metal requires `indexBufferOffset` alignment of 4
            if base_vertex == 0 && instances.start == 0 {
                //Note: for a strange reason, index alignment is not enforced here
                encoder.draw_indexed_primitives_instanced(
                    primitive_type,
                    (indices.end - indices.start) as NSUInteger,
                    index.index_type,
                    index_buffer,
                    index_offset,
                    instances.end as NSUInteger,
                );
            } else {
                assert_eq!(index_offset % WORD_ALIGNMENT, 0);
                encoder.draw_indexed_primitives_instanced_base_instance(
                    primitive_type,
                    (indices.end - indices.start) as NSUInteger,
                    index.index_type,
                    index_buffer,
                    index_offset,
                    (instances.end - instances.start) as NSUInteger,
                    base_vertex as NSInteger,
                    instances.start as NSUInteger,
                );
            }
        }
        Cmd::DrawIndirect { primitive_type, buffer, offset } => {
            encoder.draw_primitives_indirect(
                primitive_type,
                buffer.as_native(),
                offset,
            );
        }
        Cmd::DrawIndexedIndirect { primitive_type, index, buffer, offset } => {
            encoder.draw_indexed_primitives_indirect(
                primitive_type,
                index.index_type,
                index.buffer.as_native(),
                index.offset,
                buffer.as_native(),
                offset,
            );
        }
    }
}

fn exec_blit<'a>(encoder: &metal::BlitCommandEncoderRef, command: soft::BlitCommand<&'a soft::Own>) {
    use soft::BlitCommand as Cmd;
    match command {
        Cmd::CopyBuffer { src, dst, region } => {
            encoder.copy_from_buffer(
                src.as_native(),
                region.src as NSUInteger,
                dst.as_native(),
                region.dst as NSUInteger,
                region.size as NSUInteger
            );
        }
        Cmd::CopyImage { src, dst, region } => {
            let size = conv::map_extent(region.extent);
            let src_offset = conv::map_offset(region.src_offset);
            let dst_offset = conv::map_offset(region.dst_offset);
            let layers = region.src_subresource.layers.zip(region.dst_subresource.layers);
            for (src_layer, dst_layer) in layers {
                encoder.copy_from_texture(
                    src.as_native(),
                    src_layer as _,
                    region.src_subresource.level as _,
                    src_offset,
                    size,
                    dst.as_native(),
                    dst_layer as _,
                    region.dst_subresource.level as _,
                    dst_offset,
                );
            }
        }
        Cmd::CopyBufferToImage { src, dst, dst_desc, region } => {
            let extent = conv::map_extent(region.image_extent);
            let origin = conv::map_offset(region.image_offset);
            let (row_pitch, slice_pitch) = compute_pitches(&region, &dst_desc, &extent);
            let r = &region.image_layers;

            for layer in r.layers.clone() {
                let offset = region.buffer_offset + slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                encoder.copy_from_buffer_to_texture(
                    src.as_native(),
                    offset as NSUInteger,
                    row_pitch as NSUInteger,
                    slice_pitch as NSUInteger,
                    extent,
                    dst.as_native(),
                    layer as NSUInteger,
                    r.level as NSUInteger,
                    origin,
                    metal::MTLBlitOption::empty(),
                );
            }
        }
        Cmd::CopyImageToBuffer { src, src_desc, dst, region } => {
            let extent = conv::map_extent(region.image_extent);
            let origin = conv::map_offset(region.image_offset);
            let (row_pitch, slice_pitch) = compute_pitches(&region, &src_desc, &extent);
            let r = &region.image_layers;

            for layer in r.layers.clone() {
                let offset = region.buffer_offset + slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                encoder.copy_from_texture_to_buffer(
                    src.as_native(),
                    layer as NSUInteger,
                    r.level as NSUInteger,
                    origin,
                    extent,
                    dst.as_native(),
                    offset as NSUInteger,
                    row_pitch as NSUInteger,
                    slice_pitch as NSUInteger,
                    metal::MTLBlitOption::empty(),
                );
            }
        }
    }
}

fn exec_compute<'a>(encoder: &metal::ComputeCommandEncoderRef, command: soft::ComputeCommand<&'a soft::Own>) {
    use soft::ComputeCommand as Cmd;
    match command {
        Cmd::BindBuffer { index, ref buffer, offset } => {
            let native = buffer.as_ref().map(|b| b.as_native());
            encoder.set_buffer(index as _, offset, native);
        }
        Cmd::BindBufferData { words, index } => {
            encoder.set_bytes(index as _, (words.len() * WORD_SIZE) as u64, words.as_ptr() as _);
        }
        Cmd::BindTexture { index, texture } => {
            let native = texture.as_ref().map(|t| t.as_native());
            encoder.set_texture(index as _,  native);
        }
        Cmd::BindSampler { index, sampler } => {
            let native = sampler.as_ref().map(|s| s.as_native());
            encoder.set_sampler_state(index as _, native);
        }
        Cmd::BindPipeline(pipeline) => {
            encoder.set_compute_pipeline_state(pipeline);
        }
        Cmd::Dispatch { wg_size, wg_count } => {
            encoder.dispatch_thread_groups(wg_count, wg_size);
        }
        Cmd::DispatchIndirect { wg_size, buffer, offset } => {
            encoder.dispatch_thread_groups_indirect(buffer.as_native(), offset, wg_size);
        }
    }
}

fn record_commands(command_buf: &metal::CommandBufferRef, passes: &[soft::Pass]) {
    let _ap = AutoreleasePool::new(); // for encoder creation
    for pass in passes {
        match *pass {
            soft::Pass::Render { ref desc, ref commands } => {
                let encoder = command_buf.new_render_command_encoder(desc);
                for command in commands {
                    exec_render(&encoder, command.as_ref());
                }
                encoder.end_encoding();
            }
            soft::Pass::Blit(ref commands) => {
                let encoder = command_buf.new_blit_command_encoder();
                for command in commands {
                    exec_blit(&encoder, command.as_ref());
                }
                encoder.end_encoding();
            }
            soft::Pass::Compute(ref commands) => {
                let encoder = command_buf.new_compute_command_encoder();
                for command in commands {
                    exec_compute(&encoder, command.as_ref());
                }
                encoder.end_encoding();
            }
        }
    }
}

/// This is a hack around Metal System Trace logic that ignores empty command buffers entirely.
fn record_empty(command_buf: &metal::CommandBufferRef) {
    command_buf.new_blit_command_encoder().end_encoding();
}

#[derive(Default)]
struct PerformanceCounters {
    immediate_command_buffers: usize,
    deferred_command_buffers: usize,
    signal_command_buffers: usize,
    frame_wait_duration: time::Duration,
    frame_wait_count: usize,
    frame: usize,
}


pub struct CommandQueue {
    shared: Arc<Shared>,
    retained_buffers: Vec<metal::Buffer>,
    retained_textures: Vec<metal::Texture>,
    perf_counters: Option<PerformanceCounters>,
}

unsafe impl Send for CommandQueue {}
unsafe impl Sync for CommandQueue {}

impl CommandQueue {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        CommandQueue {
            shared,
            retained_buffers: Vec::new(),
            retained_textures: Vec::new(),
            perf_counters: if COUNTERS_REPORT_WINDOW != 0 {
                Some(PerformanceCounters::default())
            } else {
                None
            },
        }
    }

    fn wait<I>(&mut self, wait_semaphores: I)
    where
        I: IntoIterator,
        I::Item: Borrow<native::Semaphore>,
    {
        for semaphore in wait_semaphores {
            let sem = semaphore.borrow();
            if let Some(ref system) = sem.system {
                system.wait(!0);
            }
            if let Some(swap_image) = sem.image_ready.lock().take() {
                let start = time::Instant::now();
                let count = swap_image.wait_until_ready();
                if let Some(ref mut counters) = self.perf_counters {
                    counters.frame_wait_count += count;
                    counters.frame_wait_duration += start.elapsed();
                }
            }
        }
    }
}

impl RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<IC>(
        &mut self, submit: RawSubmission<Backend, IC>, fence: Option<&native::Fence>
    )
    where
        IC: IntoIterator,
        IC::Item: Borrow<CommandBuffer>,
    {
        let _ap = AutoreleasePool::new();
        debug!("submitting with fence {:?}", fence);

        self.wait(submit.wait_semaphores.iter().map(|&(s, _)| s));

        let queue = self.shared.queue.lock();
        let (mut num_immediate, mut num_deferred) = (0, 0);

        for buffer in submit.cmd_buffers {
            let mut inner = buffer.borrow().inner.borrow_mut();
            let CommandBufferInner {
                ref sink,
                ref mut retained_buffers,
                ref mut retained_textures,
            } = *inner;

            match *sink {
                Some(CommandSink::Immediate { ref cmd_buffer, ref token, .. }) => {
                    num_immediate += 1;
                    trace!("\timmediate {:?}", token);
                    self.retained_buffers.extend(retained_buffers.drain(..));
                    self.retained_textures.extend(retained_textures.drain(..));
                    cmd_buffer.commit();
                }
                Some(CommandSink::Deferred { ref passes, .. }) => {
                    num_deferred += 1;
                    trace!("\tdeferred with {} passes", passes.len());
                    let cmd_buffer = queue.spawn_temp();
                    cmd_buffer.enqueue();
                    cmd_buffer.set_label("deferred");
                    record_commands(&*cmd_buffer, passes);
                    cmd_buffer.commit();
                 }
                 _ => panic!("Command buffer not recorded for submission")
            }
        }

        debug!("\t{} immediate, {} deferred command buffers", num_immediate, num_deferred);
        if let Some(ref mut counters) = self.perf_counters {
            counters.immediate_command_buffers += num_immediate;
            counters.deferred_command_buffers += num_deferred;
        }

        const BLOCK_BUCKET: usize = 4;
        let system_semaphores = submit.signal_semaphores
            .into_iter()
            .filter_map(|semaphore| {
                semaphore.system.clone()
            })
            .collect::<SmallVec<[_; BLOCK_BUCKET]>>();

        // Note: completion handlers can stall the GPU, so we only make one
        // when strictly required, and collect the retained resources otherwise.
        if fence.is_some() || !system_semaphores.is_empty() {
            let moved_fence = fence.map(Arc::clone);
            let free_buffers = self.retained_buffers
                .drain(..)
                .collect::<SmallVec<[_; BLOCK_BUCKET]>>();
            let free_textures = self.retained_textures
                .drain(..)
                .collect::<SmallVec<[_; BLOCK_BUCKET]>>();

            let block = ConcreteBlock::new(move |_cb: *mut ()| -> () {
                // release the fence
                if let Some(ref f) = moved_fence {
                    *f.mutex.lock() = true;
                    f.condvar.notify_all();
                }
                // signal the semaphores
                for semaphore in &system_semaphores {
                    semaphore.signal();
                }
                // free all the manually retained resources
                let _ = free_buffers;
                let _ = free_textures;
            }).copy();

            if let Some(ref mut counters) = self.perf_counters {
                counters.signal_command_buffers += 1;
            }
            let cmd_buffer = queue.spawn_temp();
            cmd_buffer.set_label("signal");
            record_empty(cmd_buffer);
            msg_send![cmd_buffer, addCompletedHandler: block.deref() as *const _];
            cmd_buffer.commit();
        }
    }

    fn present<IS, S, IW>(&mut self, swapchains: IS, wait_semaphores: IW) -> Result<(), ()>
    where
        IS: IntoIterator<Item = (S, SwapImageIndex)>,
        S: Borrow<window::Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<native::Semaphore>,
    {
        self.wait(wait_semaphores);

        let queue = self.shared.queue.lock();
        let command_buffer = queue.raw.new_command_buffer();
        command_buffer.set_label("present");
        record_empty(command_buffer);

        for (swapchain, index) in swapchains {
            debug!("presenting frame {}", index);
            let drawable = swapchain.borrow().take_drawable(index);
            command_buffer.present_drawable(&drawable);
        }

        command_buffer.commit();

        if let Some(ref mut counters) = self.perf_counters {
            counters.frame += 1;
            if counters.frame >= COUNTERS_REPORT_WINDOW {
                let time = counters.frame_wait_duration / counters.frame as u32;
                let total_submitted =
                    counters.immediate_command_buffers +
                    counters.deferred_command_buffers +
                    counters.signal_command_buffers;
                println!("Performance counters:");
                println!("\tCommand buffers: {} immediate, {} deferred, {} signals",
                    counters.immediate_command_buffers / counters.frame,
                    counters.deferred_command_buffers / counters.frame,
                    counters.signal_command_buffers / counters.frame,
                );
                println!("\tEstimated pipeline length is {} frames, given the total active {} command buffers",
                    counters.frame * queue.reserve.start / total_submitted.max(1),
                    queue.reserve.start,
                );
                println!("\tFrame wait time is {}ms over {} requests",
                    time.as_secs() as u32 * 1000 + time.subsec_millis(),
                    counters.frame_wait_count as f32 / counters.frame as f32,
                );
                *counters = PerformanceCounters::default();
            }
        }

        Ok(())
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        QueueInner::wait_idle(&self.shared.queue);
        Ok(())
    }
}

impl pool::RawCommandPool<Backend> for CommandPool {
    fn reset(&mut self) {
        for cmd_buffer in &self.allocated {
            cmd_buffer
                .borrow_mut()
                .reset(&self.shared);
        }
    }

    fn allocate(
        &mut self, num: usize, _level: com::RawLevel
    ) -> Vec<CommandBuffer> {
        //TODO: fail with OOM if we allocate more actual command buffers
        // than our mega-queue supports.
        //TODO: Implement secondary buffers
        let buffers: Vec<_> = (0..num).map(|_| CommandBuffer {
            inner: Arc::new(RefCell::new(CommandBufferInner {
                sink: None,
                retained_buffers: Vec::new(),
                retained_textures: Vec::new(),
            })),
            shared: self.shared.clone(),
            state: State {
                viewport: None,
                scissors: None,
                blend_color: None,
                render_pso: None,
                render_pso_is_compatible: false,
                compute_pso: None,
                work_group_size: MTLSize { width: 0, height: 0, depth: 0 },
                primitive_type: MTLPrimitiveType::Point,
                resources_vs: StageResources::new(),
                resources_fs: StageResources::new(),
                resources_cs: StageResources::new(),
                index_buffer: None,
                rasterizer_state: None,
                depth_bias: pso::DepthBias::default(),
                stencil: native::StencilState {
                    front_reference: 0,
                    back_reference: 0,
                    front_read_mask: !0,
                    back_read_mask: !0,
                    front_write_mask: !0,
                    back_write_mask: !0,
                },
                push_constants: Vec::new(),
                vertex_buffers: Vec::new(),
                framebuffer_inner: native::FramebufferInner {
                    extent: Extent::default(),
                    aspects: Aspects::empty(),
                    colors: SmallVec::new(),
                    depth_stencil: None,
                }
            },
            temp: Temp {
                clear_vertices: Vec::new(),
                blit_vertices: FastHashMap::default(),
            },
        }).collect();

        self.allocated.extend(buffers.iter().map(|buf| buf.inner.clone()));
        buffers
    }

    /// Free command buffers which are allocated from this pool.
    unsafe fn free(&mut self, mut buffers: Vec<CommandBuffer>) {
        use hal::command::RawCommandBuffer;
        for buf in &mut buffers {
            buf.reset(true);
        }
        for cmd_buf in buffers {
            match self.allocated.iter_mut().position(|b| Arc::ptr_eq(b, &cmd_buf.inner)) {
                Some(index) => {
                    self.allocated.swap_remove(index);
                }
                None => {
                    error!("Unable to free a command buffer!")
                }
            }
        }
    }
}

/// Sets up the load/store operations. Returns `true` if the clear color needs to be set.
fn set_operations(attachment: &metal::RenderPassAttachmentDescriptorRef, ops: AttachmentOps) -> AttachmentLoadOp {
    attachment.set_load_action(conv::map_load_operation(ops.load));
    attachment.set_store_action(conv::map_store_operation(ops.store));
    ops.load
}

impl CommandBuffer {
    fn update_depth_stencil(&self) {
        let mut inner = self.inner.borrow_mut();
        let mut pre = inner.sink().pre_render();
        if !pre.is_void() {
            let mut pipes = self.shared.service_pipes.lock();
            if let Some(com) = self.state.sync_depth_stencil(&mut *pipes, &self.shared.device) {
                pre.issue(com)
            }
        }
    }
}

impl com::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self, flags: com::CommandBufferFlags, _info: com::CommandBufferInheritanceInfo<Backend>) {
        self.reset(false);
        //TODO: Implement secondary command buffers
        let sink = if flags.contains(com::CommandBufferFlags::ONE_TIME_SUBMIT) {
            let (cmd_buffer, token) = self.shared.queue.lock().spawn();
            CommandSink::Immediate {
                cmd_buffer,
                token,
                encoder_state: EncoderState::None,
            }
        } else {
            CommandSink::Deferred {
                passes: Vec::new(),
                is_encoding: false,
            }
        };

        self.inner.borrow_mut().sink = Some(sink);
        self.state.reset_resources();
    }

    fn finish(&mut self) {
        self.inner
            .borrow_mut()
            .sink()
            .stop_encoding();
    }

    fn reset(&mut self, _release_resources: bool) {
        self.state.reset_resources();
        self.inner
            .borrow_mut()
            .reset(&self.shared);
    }

    fn pipeline_barrier<'a, T>(
        &mut self,
        _stages: Range<pso::PipelineStage>,
        _dependencies: memory::Dependencies,
        _barriers: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<memory::Barrier<'a, Backend>>,
    {
        // TODO: MTLRenderCommandEncoder.textureBarrier on macOS?
    }

    fn fill_buffer<R>(
        &mut self,
        buffer: &native::Buffer,
        range: R,
        data: u32,
    ) where
        R: RangeArg<buffer::Offset>,
    {
        let mut inner = self.inner.borrow_mut();
        let pipes = self.shared.service_pipes.lock();
        let pso = pipes.get_fill_buffer();

        let start = *range.start().unwrap_or(&0);
        assert_eq!(start % WORD_ALIGNMENT, 0);

        let end = match range.end() {
            Some(e) => {
                assert_eq!(e % WORD_ALIGNMENT, 0);
                *e
            },
            None => {
                let len = buffer.raw.length();
                len - len % WORD_ALIGNMENT
            },
        };

        let length = (end - start) / WORD_ALIGNMENT;
        let value_and_length = [data, length as _];

        // TODO: Consider writing multiple values per thread in shader
        let threads_per_threadgroup = pso.thread_execution_width();
        let threadgroups = (length + threads_per_threadgroup - 1) / threads_per_threadgroup;

        let wg_count = MTLSize {
            width: threadgroups,
            height: 1,
            depth: 1,
        };
        let wg_size = MTLSize {
            width: threads_per_threadgroup,
            height: 1,
            depth: 1,
        };

        let commands = [
            soft::ComputeCommand::BindPipeline(pso),
            soft::ComputeCommand::BindBuffer {
                index: 0,
                buffer: Some(BufferPtr(buffer.raw.as_ptr())),
                offset: start,
            },
            soft::ComputeCommand::BindBufferData {
                index: 1,
                words: &value_and_length[..],
            },
            soft::ComputeCommand::Dispatch {
                wg_size,
                wg_count,
            },
        ];

        inner.sink().begin_compute_pass(
            PassDoor::Closed { label: "fill_buffer" },
            commands.iter().cloned(),
        );
    }

    fn update_buffer(
        &mut self,
        dst: &native::Buffer,
        offset: buffer::Offset,
        data: &[u8],
    ) {
        let src = self.shared.device
            .lock()
            .new_buffer_with_data(
                data.as_ptr() as _,
                data.len() as _,
                metal::MTLResourceOptions::CPUCacheModeWriteCombined,
            );
        src.set_label("update_buffer");

        let mut inner = self.inner.borrow_mut();
        {
            let command = soft::BlitCommand::CopyBuffer {
                src: BufferPtr(src.as_ptr()),
                dst: BufferPtr(dst.raw.as_ptr()),
                region: com::BufferCopy {
                    src: 0,
                    dst: offset,
                    size: data.len() as _,
                },
            };

            inner
                .sink()
                .blit_commands(iter::once(command));
        }

        inner.retained_buffers.push(src);
    }

    fn clear_image<T>(
        &mut self,
        image: &native::Image,
        _layout: Layout,
        color: com::ClearColorRaw,
        depth_stencil: com::ClearDepthStencilRaw,
        subresource_ranges: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<SubresourceRange>,
    {
        let _ap = AutoreleasePool::new();

        let CommandBufferInner {
            ref mut retained_textures,
            ref mut sink,
            ..
        } = *self.inner.borrow_mut();

        let clear_color = image.shader_channel.interpret(color);
        let base_extent = image.kind.extent();

        for subresource_range in subresource_ranges {
            let sub = subresource_range.borrow();
            let descriptor = metal::RenderPassDescriptor::new();

            let num_layers = (sub.layers.end - sub.layers.start) as u64;
            let layers = if CLEAR_IMAGE_ARRAY {
                0 .. 1
            } else {
                sub.layers.clone()
            };
            let texture = if CLEAR_IMAGE_ARRAY && sub.layers.start > 0 {
                // aliasing is necessary for bulk-clearing all layers starting with 0
                let tex = image.raw.new_texture_view_from_slice(
                    image.mtl_format,
                    image.mtl_type,
                    NSRange {
                        location: 0,
                        length: image.raw.mipmap_level_count(),
                    },
                    NSRange {
                        location: sub.layers.start as _,
                        length: num_layers,
                    },
                );
                retained_textures.push(tex);
                retained_textures.last().unwrap()
            } else {
                &*image.raw
            };

            let color_attachment = if image.format_desc.aspects.contains(Aspects::COLOR) {
                let attachment = descriptor
                    .color_attachments()
                    .object_at(0)
                    .unwrap();
                attachment.set_texture(Some(texture));
                attachment.set_store_action(metal::MTLStoreAction::Store);
                if sub.aspects.contains(Aspects::COLOR) {
                    attachment.set_load_action(metal::MTLLoadAction::Clear);
                    attachment.set_clear_color(clear_color.clone());
                    Some(attachment)
                } else {
                    attachment.set_load_action(metal::MTLLoadAction::Load);
                    None
                }
            } else {
                assert!(!sub.aspects.contains(Aspects::COLOR));
                None
            };

            let depth_attachment = if image.format_desc.aspects.contains(Aspects::DEPTH) {
                let attachment = descriptor
                    .depth_attachment()
                    .unwrap();
                attachment.set_texture(Some(texture));
                attachment.set_store_action(metal::MTLStoreAction::Store);
                if sub.aspects.contains(Aspects::DEPTH) {
                    attachment.set_load_action(metal::MTLLoadAction::Clear);
                    attachment.set_clear_depth(depth_stencil.depth as _);
                    Some(attachment)
                } else {
                    attachment.set_load_action(metal::MTLLoadAction::Load);
                    None
                }
            } else {
                assert!(!sub.aspects.contains(Aspects::DEPTH));
                None
            };

            let stencil_attachment = if image.format_desc.aspects.contains(Aspects::STENCIL) {
                let attachment = descriptor
                    .stencil_attachment()
                    .unwrap();
                attachment.set_texture(Some(texture));
                attachment.set_store_action(metal::MTLStoreAction::Store);
                if sub.aspects.contains(Aspects::STENCIL) {
                    attachment.set_load_action(metal::MTLLoadAction::Clear);
                    attachment.set_clear_stencil(depth_stencil.stencil);
                    Some(attachment)
                } else {
                    attachment.set_load_action(metal::MTLLoadAction::Load);
                    None
                }
            } else {
                assert!(!sub.aspects.contains(Aspects::STENCIL));
                None
            };

            for layer in layers {
                for level in sub.levels.clone() {
                    if base_extent.depth > 1 {
                        assert_eq!(sub.layers.end, 1);
                        let depth = base_extent.at_level(level).depth as u64;
                        descriptor.set_render_target_array_length(depth);
                    } else if CLEAR_IMAGE_ARRAY {
                        descriptor.set_render_target_array_length(num_layers);
                    };

                    if let Some(attachment) = color_attachment {
                        attachment.set_level(level as _);
                        if !CLEAR_IMAGE_ARRAY {
                            attachment.set_slice(layer as _);
                        }
                    }
                    if let Some(attachment) = depth_attachment {
                        attachment.set_level(level as _);
                        if !CLEAR_IMAGE_ARRAY {
                            attachment.set_slice(layer as _);
                        }
                    }
                    if let Some(attachment) = stencil_attachment {
                        attachment.set_level(level as _);
                        if !CLEAR_IMAGE_ARRAY {
                            attachment.set_slice(layer as _);
                        }
                    }

                    sink.as_mut()
                        .unwrap()
                        .begin_render_pass(
                            PassDoor::Closed { label: "clear_image" },
                            descriptor,
                            iter::empty(),
                        );
                    // no actual pass body - everything is in the attachment clear operations
                }
            }
        }
    }

    fn clear_attachments<T, U>(
        &mut self,
        clears: T,
        rects: U,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::ClearRect>,
    {
        // gather vertices/polygons
        let de = self.state.framebuffer_inner.extent;
        let vertices = &mut self.temp.clear_vertices;
        vertices.clear();

        for rect in rects {
            let r = rect.borrow();
            for layer in r.layers.clone() {
                let data = [
                    [
                        r.rect.x,
                        r.rect.y,
                    ],
                    [
                        r.rect.x,
                        r.rect.y + r.rect.h,
                    ],
                    [
                        r.rect.x + r.rect.w,
                        r.rect.y + r.rect.h,
                    ],
                    [
                        r.rect.x + r.rect.w,
                        r.rect.y,
                    ],
                ];
                // now use the hard-coded index array to add 6 vertices to the list
                //TODO: could use instancing here
                // - with triangle strips
                // - with half of the data supplied per instance

                for &index in &[0usize, 1, 2, 2, 3, 0] {
                    let d = data[index];
                    vertices.push(ClearVertex {
                        pos: [
                            d[0] as f32 / de.width as f32,
                            d[1] as f32 / de.height as f32,
                            0.0, //TODO: depth Z
                            layer as f32,
                        ],
                    });
                }
            }
        }

        let mut vertex_is_dirty = true;
        let mut inner = self.inner.borrow_mut();

        //  issue a PSO+color switch and a draw for each requested clear
        let mut pipes = self.shared.service_pipes.lock();

        for clear in clears {
            let mut key = ClearKey {
                framebuffer_aspects: self.state.framebuffer_inner.aspects,
                color_formats: [metal::MTLPixelFormat::Invalid; 1],
                depth_stencil_format: self.state.framebuffer_inner.depth_stencil
                    .unwrap_or(metal::MTLPixelFormat::Invalid),
                target_index: None,
            };
            for (out, cat) in key.color_formats.iter_mut().zip(&self.state.framebuffer_inner.colors) {
                *out = cat.mtl_format;
            }

            let (aspects, com_clear) = match *clear.borrow() {
                com::AttachmentClear::Color { index, value } => {
                    let cat = &self.state.framebuffer_inner.colors[index];
                    //Note: technically we should be able to derive the Channel from the
                    // `value` variant, but this is blocked by the portability that is
                    // always passing the attachment clears as `ClearColor::Float` atm.
                    let raw_value = com::ClearColorRaw::from(value);
                    let com = soft::RenderCommand::BindBufferData {
                        stage: pso::Stage::Fragment,
                        index: 0,
                        words: unsafe { slice::from_raw_parts(
                            raw_value.float32.as_ptr() as *const u32,
                            mem::size_of::<com::ClearColorRaw>() / WORD_SIZE,
                        )},
                    };
                    key.target_index = Some((index as u8, cat.channel));
                    (Aspects::COLOR, Some(com))
                }
                com::AttachmentClear::DepthStencil { depth, stencil } => {
                    let mut aspects = Aspects::empty();
                    if let Some(value) = depth {
                        for v in vertices.iter_mut() {
                            v.pos[2] = value;
                        }
                        vertex_is_dirty = true;
                        aspects |= Aspects::DEPTH;
                    }
                    if let Some(_) = stencil {
                        //TODO: soft::RenderCommand::SetStencilReference
                        aspects |= Aspects::STENCIL;
                    }
                    (aspects, None)
                }
            };

            let com_vertex = if vertex_is_dirty {
                vertex_is_dirty = false;
                Some(soft::RenderCommand::BindBufferData {
                    stage: pso::Stage::Vertex,
                    index: 0,
                    words: unsafe {
                        slice::from_raw_parts(
                            vertices.as_ptr() as *const u32,
                            vertices.len() * mem::size_of::<ClearVertex>() / WORD_SIZE
                        )
                    }
                })
            } else {
                None
            };

            let ServicePipes {
                ref library,
                ref depth_stencil_states,
                ref mut clears,
                ..
            } = *pipes;

            let clear_pso = clears.get(key, library, &self.shared.device);
            let com_pso = iter::once(soft::RenderCommand::BindPipeline(clear_pso, None));

            let com_ds = if !aspects.contains(Aspects::COLOR) {
                Some(soft::RenderCommand::SetDepthStencilState(
                    depth_stencil_states.get_write(aspects)
                ))
            } else {
                None
            };

            let commands = com_clear
                .into_iter()
                .chain(com_vertex)
                .chain(com_pso)
                .chain(com_ds)
                .chain(iter::once(soft::RenderCommand::Draw {
                    primitive_type: MTLPrimitiveType::Triangle,
                    vertices: 0 .. vertices.len() as _,
                    instances: 0 .. 1,
                }));

            inner.sink().render_commands(commands);
        }

        // reset all the affected states
        let com_pso = if let Some(ref ps) = self.state.render_pso {
            if self.state.render_pso_is_compatible {
                Some(soft::RenderCommand::BindPipeline(&*ps.raw, None))
            } else {
                warn!("Not restoring the current PSO after clear_attachments because it's not compatible");
                None
            }
        } else {
            None
        };

        let com_ds = self.state.sync_depth_stencil(&mut *pipes, &self.shared.device);

        let com_vs = if let Some(&Some((buffer, offset))) = self.state.resources_vs.buffers.first() {
            Some(soft::RenderCommand::BindBuffer {
                stage: pso::Stage::Vertex,
                index: 0,
                buffer: Some(buffer),
                offset,
            })
        } else {
            None
        };
        let com_fs = if let Some(&Some((buffer, offset))) = self.state.resources_fs.buffers.first() {
            Some(soft::RenderCommand::BindBuffer {
                stage: pso::Stage::Fragment,
                index: 0,
                buffer: Some(buffer),
                offset,
            })
        } else {
            None
        };

        let commands = com_pso
            .into_iter()
            .chain(com_ds)
            .chain(com_vs)
            .chain(com_fs);
        inner.sink().render_commands(commands);

        vertices.clear();
    }

    fn resolve_image<T>(
        &mut self,
        _src: &native::Image,
        _src_layout: Layout,
        _dst: &native::Image,
        _dst_layout: Layout,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageResolve>,
    {
        unimplemented!()
    }

    fn blit_image<T>(
        &mut self,
        src: &native::Image,
        _src_layout: Layout,
        dst: &native::Image,
        _dst_layout: Layout,
        filter: Filter,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageBlit>
    {
        let _ap = AutoreleasePool::new();

        let vertices = &mut self.temp.blit_vertices;
        vertices.clear();

        for region in regions {
            let r = region.borrow();

            // layer count must be equal in both subresources
            debug_assert_eq!(r.src_subresource.layers.len(), r.dst_subresource.layers.len());
            debug_assert_eq!(r.src_subresource.aspects, r.dst_subresource.aspects);
            debug_assert!(src.format_desc.aspects.contains(r.src_subresource.aspects));
            debug_assert!(dst.format_desc.aspects.contains(r.dst_subresource.aspects));

            let se = src.kind.extent().at_level(r.src_subresource.level);
            let de = dst.kind.extent().at_level(r.dst_subresource.level);
            //TODO: support 3D textures
            if se.depth != 1 || de.depth != 1 {
                warn!("3D image blits are not supported properly yet: {:?} -> {:?}", se, de);
            }

            let layers = r.src_subresource.layers.clone().zip(r.dst_subresource.layers.clone());
            let list = vertices
                .entry((r.dst_subresource.aspects, r.dst_subresource.level))
                .or_insert(Vec::new());

            for (src_layer, dst_layer) in layers {
                // this helper array defines unique data for quad vertices
                let data = [
                    [
                        r.src_bounds.start.x,
                        r.src_bounds.start.y,
                        r.dst_bounds.start.x,
                        r.dst_bounds.start.y,
                    ],
                    [
                        r.src_bounds.start.x,
                        r.src_bounds.end.y,
                        r.dst_bounds.start.x,
                        r.dst_bounds.end.y,
                    ],
                    [
                        r.src_bounds.end.x,
                        r.src_bounds.end.y,
                        r.dst_bounds.end.x,
                        r.dst_bounds.end.y,
                    ],
                    [
                        r.src_bounds.end.x,
                        r.src_bounds.start.y,
                        r.dst_bounds.end.x,
                        r.dst_bounds.start.y,
                    ],
                ];
                // now use the hard-coded index array to add 6 vertices to the list
                //TODO: could use instancing here
                // - with triangle strips
                // - with half of the data supplied per instance

                for &index in &[0usize, 1, 2, 2, 3, 0] {
                    let d = data[index];
                    list.push(BlitVertex {
                        uv: [
                            d[0] as f32 / se.width as f32,
                            d[1] as f32 / se.height as f32,
                            src_layer as f32,
                            r.src_subresource.level as f32,
                        ],
                        pos: [
                            d[2] as f32 / de.width as f32,
                            d[3] as f32 / de.height as f32,
                            0.0,
                            dst_layer as f32,
                        ],
                    });
                }
            }
        }

        let descriptor = metal::RenderPassDescriptor::new();
        if src.format_desc.aspects.contains(Aspects::COLOR) {
            descriptor
                .color_attachments()
                .object_at(0)
                .unwrap()
                .set_texture(Some(&dst.raw));
        }
        if src.format_desc.aspects.contains(Aspects::DEPTH) {
            descriptor
                .depth_attachment()
                .unwrap()
                .set_texture(Some(&dst.raw));
        }
        if src.format_desc.aspects.contains(Aspects::STENCIL) {
            descriptor
                .stencil_attachment()
                .unwrap()
                .set_texture(Some(&dst.raw));
        }

        let mut inner = self.inner.borrow_mut();
        // Note: we don't bother to restore any render states here, since we are currently
        // outside of a render pass, and the state will be reset automatically once
        // we enter the next pass.
        let mut pipes = self.shared.service_pipes.lock();
        let ServicePipes {
            ref library,
            ref sampler_states,
            ref depth_stencil_states,
            ref mut blits,
            ..
        } = *pipes;

        let key = (dst.mtl_type, dst.mtl_format, src.format_desc.aspects, dst.shader_channel);

        let prelude = [
            soft::RenderCommand::BindPipeline(
                blits.get(key, library, &self.shared.device),
                None,
            ),
            soft::RenderCommand::BindSampler {
                stage: pso::Stage::Fragment,
                index: 0,
                sampler: Some(SamplerPtr(sampler_states.get(filter).as_ptr())),
            },
            soft::RenderCommand::BindTexture {
                stage: pso::Stage::Fragment,
                index: 0,
                texture: Some(TexturePtr(src.raw.as_ptr()))
            },
        ];

        let com_ds = if src.format_desc.aspects.intersects(Aspects::DEPTH | Aspects::STENCIL) {
            Some(soft::RenderCommand::SetDepthStencilState(
                depth_stencil_states.get_write(src.format_desc.aspects)
            ))
        } else {
            None
        };

        for ((aspects, level), list) in vertices.drain() {
            let ext = dst.kind.extent().at_level(level);

            let extra = [
                //Note: flipping Y coordinate of the destination here
                soft::RenderCommand::SetViewport(MTLViewport {
                    originX: 0.0,
                    originY: ext.height as _,
                    width: ext.width as _,
                    height: -(ext.height as f64),
                    znear: 0.0,
                    zfar: 1.0,
                }),
                soft::RenderCommand::SetScissor(MTLScissorRect {
                    x: 0,
                    y: 0,
                    width: ext.width as _,
                    height: ext.height as _,
                }),
                soft::RenderCommand::BindBufferData {
                    stage: pso::Stage::Vertex,
                    index: 0,
                    words: unsafe {
                        slice::from_raw_parts(
                            list.as_ptr() as *const u32,
                            list.len() * mem::size_of::<BlitVertex>() / WORD_SIZE
                        )
                    }
                },
                soft::RenderCommand::Draw {
                    primitive_type: MTLPrimitiveType::Triangle,
                    vertices: 0 .. list.len() as _,
                    instances: 0 .. 1,
                },
            ];

            descriptor.set_render_target_array_length(ext.depth as _);
            if aspects.contains(Aspects::COLOR) {
                descriptor
                    .color_attachments()
                    .object_at(0)
                    .unwrap()
                    .set_level(level as _);
            }
            if aspects.contains(Aspects::DEPTH) {
                descriptor
                    .depth_attachment()
                    .unwrap()
                    .set_level(level as _);
            }
            if aspects.contains(Aspects::STENCIL) {
                descriptor
                    .stencil_attachment()
                    .unwrap()
                    .set_level(level as _);
            }

            let commands = prelude
                .iter()
                .chain(&com_ds)
                .chain(&extra)
                .cloned();

            inner
                .sink()
                .begin_render_pass(
                    PassDoor::Closed { label: "blit_image" },
                    &descriptor,
                    commands,
                );
        }
    }

    fn bind_index_buffer(&mut self, view: buffer::IndexBufferView<Backend>) {
        let buffer = view.buffer.raw.clone();
        let offset = view.offset;
        let index_type = conv::map_index_type(view.index_type);
        self.state.index_buffer = Some(IndexBuffer {
            buffer: BufferPtr(buffer.as_ptr()),
            offset,
            index_type,
        });
    }


    fn bind_vertex_buffers<I, T>(&mut self, first_binding: u32, buffers: I)
    where
        I: IntoIterator<Item = (T, buffer::Offset)>,
        T: Borrow<native::Buffer>,
    {
        if self.state.vertex_buffers.len() <= first_binding as usize {
            self.state.vertex_buffers.resize(first_binding as usize + 1, None);
        }
        for (i, (buffer, offset)) in buffers.into_iter().enumerate() {
            let b = buffer.borrow();
            let buffer_ptr = BufferPtr(b.raw.as_ptr());
            let index = first_binding as usize + i;
            let value = Some((buffer_ptr, b.range.start + offset));
            if index >= self.state.vertex_buffers.len() {
                debug_assert_eq!(index, self.state.vertex_buffers.len());
                self.state.vertex_buffers.push(value);
            } else {
                self.state.vertex_buffers[index] = value;
            }
        }

        let mask = self.state.set_vertex_buffers();
        if mask != 0 {
            let mut inner = self.inner.borrow_mut();
            let mut pre = inner.sink().pre_render();
            if !pre.is_void() {
                for com in self.state.iter_vertex_buffers(mask) {
                    pre.issue(com);
                }
            }
        }
    }

    fn set_viewports<T>(&mut self, first_viewport: u32, vps: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        // macOS_GPUFamily1_v3 supports >1 viewport, todo
        if first_viewport != 0 {
            panic!("First viewport != 0; Metal supports only one viewport");
        }
        let mut vps = vps.into_iter();
        let vp_borrowable = vps.next().expect("No viewport provided, Metal supports exactly one");
        let vp = vp_borrowable.borrow();
        if vps.next().is_some() {
            // TODO should we panic here or set buffer in an erronous state?
            panic!("More than one viewport set; Metal supports only one viewport");
        }

        let com = self.state.set_viewport(vp, &self.shared.disabilities);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render()
            .issue(com);
    }

    fn set_scissors<T>(&mut self, first_scissor: u32, rects: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        // macOS_GPUFamily1_v3 supports >1 scissor/viewport, todo
        if first_scissor != 0 {
            panic!("First scissor != 0; Metal supports only one viewport");
        }
        let mut rects = rects.into_iter();
        let rect_borrowable = rects.next().expect("No scissor provided, Metal supports exactly one");
        let rect = rect_borrowable.borrow();
        if rects.next().is_some() {
            panic!("More than one scissor set; Metal supports only one viewport");
        }

        let com = self.state.set_scissor(rect);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render()
            .issue(com);
    }

    fn set_blend_constants(&mut self, color: pso::ColorValue) {
        let com = self.state.set_blend_color(&color);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render()
            .issue(com);
    }

    fn set_depth_bounds(&mut self, _: Range<f32>) {
        warn!("Depth bounds test is not supported");
    }

    fn set_line_width(&mut self, width: f32) {
        validate_line_width(width);
    }

    fn set_depth_bias(&mut self, depth_bias: pso::DepthBias) {
        let com = self.state.set_depth_bias(&depth_bias);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render()
            .issue(com);
    }

    fn set_stencil_reference(&mut self, faces: pso::Face, value: pso::StencilValue) {
        if faces.contains(pso::Face::FRONT) {
            self.state.stencil.front_reference = value;
        }
        if faces.contains(pso::Face::BACK) {
            self.state.stencil.back_reference = value;
        }

        let com = soft::RenderCommand::SetStencilReferenceValues(
            self.state.stencil.front_reference,
            self.state.stencil.back_reference,
        );
        self.inner
            .borrow_mut()
            .sink()
            .pre_render()
            .issue(com);
    }

    fn set_stencil_read_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        if faces.contains(pso::Face::FRONT) {
            self.state.stencil.front_read_mask = value;
        }
        if faces.contains(pso::Face::BACK) {
            self.state.stencil.back_read_mask = value;
        }
        self.update_depth_stencil();
    }

    fn set_stencil_write_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        if faces.contains(pso::Face::FRONT) {
            self.state.stencil.front_write_mask = value;
        }
        if faces.contains(pso::Face::BACK) {
            self.state.stencil.back_write_mask = value;
        }
        self.update_depth_stencil();
    }

    fn begin_render_pass<T>(
        &mut self,
        render_pass: &native::RenderPass,
        framebuffer: &native::Framebuffer,
        _render_area: pso::Rect,
        clear_values: T,
        _first_subpass: com::SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ClearValueRaw>,
    {
        // FIXME: subpasses
        let _ap = AutoreleasePool::new();

        // we are going to modify the RP descriptor here, so
        // locking to avoid data races.
        let descriptor = framebuffer.descriptor.lock();

        let mut num_colors = 0;
        let mut full_aspects = Aspects::empty();
        let mut inner = self.inner.borrow_mut();

        let dummy_value = com::ClearValueRaw {
            color: com:: ClearColorRaw {
                int32: [0; 4],
            },
        };
        let clear_values_iter = clear_values
            .into_iter()
            .map(|c| *c.borrow())
            .chain(iter::repeat(dummy_value));

        for (rat, clear_value) in render_pass.attachments.iter().zip(clear_values_iter) {
            let (aspects, channel) = match rat.format {
                Some(format) => (format.surface_desc().aspects, Channel::from(format.base_format().1)),
                None => continue,
            };
            full_aspects |= aspects;
            if aspects.contains(Aspects::COLOR) {
                let color_desc = descriptor
                    .color_attachments()
                    .object_at(num_colors)
                    .unwrap();
                if set_operations(color_desc, rat.ops) == AttachmentLoadOp::Clear {
                    let mtl_color = channel
                        .interpret(unsafe { clear_value.color });
                    color_desc.set_clear_color(mtl_color);
                }
                num_colors += 1;
            }
            if aspects.contains(Aspects::DEPTH) {
                let depth_desc = descriptor.depth_attachment().unwrap();
                if set_operations(depth_desc, rat.ops) == AttachmentLoadOp::Clear {
                    let mtl_depth = unsafe { clear_value.depth_stencil.depth as f64 };
                    depth_desc.set_clear_depth(mtl_depth);
                }
            }
            if aspects.contains(Aspects::STENCIL) {
                let stencil_desc = descriptor.stencil_attachment().unwrap();
                if set_operations(stencil_desc, rat.stencil_ops) == AttachmentLoadOp::Clear {
                    let mtl_stencil = unsafe { clear_value.depth_stencil.stencil };
                    stencil_desc.set_clear_stencil(mtl_stencil);
                }
            }
        }

        self.state.render_pso_is_compatible = match self.state.render_pso {
            Some(ref ps) => ps.at_formats.len() == render_pass.attachments.len() &&
                ps.at_formats.iter().zip(&render_pass.attachments).all(|(f, at)| *f == at.format),
            _ => false
        };

        self.state.framebuffer_inner = framebuffer.inner.clone();
        let mut pipes = self.shared.service_pipes.lock();
        let com_ds = if full_aspects.intersects(Aspects::DEPTH | Aspects::STENCIL) {
            self.state.sync_depth_stencil(&mut *pipes, &self.shared.device)
        } else {
            None
        };
        let init_commands = self.state
            .make_render_commands(full_aspects)
            .chain(com_ds);

        inner
            .sink()
            .begin_render_pass(PassDoor::Open, &*descriptor, init_commands);
    }

    fn next_subpass(&mut self, _contents: com::SubpassContents) {
        unimplemented!()
    }

    fn end_render_pass(&mut self) {
        self.inner
            .borrow_mut()
            .sink()
            .stop_encoding();
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        let mut set_stencil_references = false;
        if let pso::StencilTest::On { ref front, ref back } = pipeline.depth_stencil_desc.stencil {
            if let pso::State::Static(value) = front.mask_read {
                self.state.stencil.front_read_mask = value;
            }
            if let pso::State::Static(value) = front.mask_write {
                self.state.stencil.front_write_mask = value;
            }
            if let pso::State::Static(value) = front.reference {
                self.state.stencil.front_reference = value;
                set_stencil_references = true;
            }
            if let pso::State::Static(value) = back.mask_read {
                self.state.stencil.back_read_mask = value;
            }
            if let pso::State::Static(value) = back.mask_write {
                self.state.stencil.back_write_mask = value;
            }
            if let pso::State::Static(value) = back.reference {
                self.state.stencil.back_reference = value;
                set_stencil_references = true;
            }
        }

        self.state.render_pso_is_compatible = true; //assume good intent :)
        self.state.render_pso = Some(RenderPipelineState {
            raw: pipeline.raw.to_owned(),
            ds_desc: pipeline.depth_stencil_desc.clone(),
            vbuf_map: pipeline.vertex_buffer_map.clone(),
            at_formats: pipeline.attachment_formats.clone(),
        });
        self.state.rasterizer_state = pipeline.rasterizer_state.clone();
        self.state.primitive_type = pipeline.primitive_type;
        let vertex_mask = self.state.set_vertex_buffers();

        let mut inner = self.inner.borrow_mut();
        let mut pre = inner.sink().pre_render();

        pre.issue(soft::RenderCommand::BindPipeline(
            &*pipeline.raw,
            pipeline.rasterizer_state.clone(),
        ));

        let mut pipes = self.shared.service_pipes.lock();
        pre.issue(self.state.sync_depth_stencil(&mut *pipes, &self.shared.device).unwrap());
        if set_stencil_references {
            pre.issue(soft::RenderCommand::SetStencilReferenceValues(
                self.state.stencil.front_reference,
                self.state.stencil.back_reference,
            ));
        }
        if let pso::State::Static(value) = pipeline.depth_bias {
            self.state.depth_bias = value;
            pre.issue(soft::RenderCommand::SetDepthBias(value));
        }

        if let Some(ref vp) = pipeline.baked_states.viewport {
            pre.issue(self.state.set_viewport(vp, &self.shared.disabilities));
        }
        if let Some(ref rect) = pipeline.baked_states.scissor {
            pre.issue(self.state.set_scissor(rect));
        }
        if let Some(ref color) = pipeline.baked_states.blend_color {
            pre.issue(self.state.set_blend_color(color));
        }

        // re-bind vertex buffers
        if vertex_mask != 0 {
            for command in self.state.iter_vertex_buffers(vertex_mask) {
                pre.issue(command);
            }
        }
    }

    fn bind_graphics_descriptor_sets<'a, I, J>(
        &mut self,
        pipe_layout: &native::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<native::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<com::DescriptorSetOffset>,
    {
        use spirv_cross::{msl, spirv};

        let mut offset_iter = offsets.into_iter();
        let mut dynamic_offsets = SmallVec::<[u64; 16]>::new();
        let mut inner = self.inner.borrow_mut();
        let mut pre = inner.sink().pre_render();

        for (set_index, desc_set) in sets.into_iter().enumerate() {
            match *desc_set.borrow() {
                native::DescriptorSet::Emulated { ref pool, ref layouts, ref sampler_range, ref texture_range, ref buffer_range } => {
                    let data = pool.read();
                    let mut sampler_base = sampler_range.start as usize;
                    let mut texture_base = texture_range.start as usize;
                    let mut buffer_base = buffer_range.start as usize;

                    for layout in layouts.iter() {
                        let sm_range = sampler_base .. sampler_base + layout.count;
                        let tx_range = texture_base .. texture_base + layout.count;
                        let bf_range = buffer_base .. buffer_base + layout.count;
                        native::DescriptorPool::count_bindings(layout.ty, layout.count,
                            &mut sampler_base, &mut texture_base, &mut buffer_base);

                        if buffer_base != bf_range.start {
                            dynamic_offsets.clear();
                            for bref in &data.buffers[bf_range.clone()] {
                                if bref.base.is_some() && bref.dynamic {
                                    dynamic_offsets.push(*offset_iter
                                        .next()
                                        .expect("No dynamic offset provided!")
                                        .borrow() as u64
                                    );
                                }
                            }
                        }

                        // collect the binding stages
                        let bind_vs = if layout.stage_flags.contains(pso::ShaderStageFlags::VERTEX) {
                            let loc = msl::ResourceBindingLocation {
                                stage: spirv::ExecutionModel::Vertex,
                                desc_set: (first_set + set_index) as _,
                                binding: layout.binding as _,
                            };
                            Some((pso::Stage::Vertex, loc, &mut self.state.resources_vs))
                        } else {
                            None
                        };
                        let bind_fs = if layout.stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                            let loc = msl::ResourceBindingLocation {
                                stage: spirv::ExecutionModel::Fragment,
                                desc_set: (first_set + set_index) as _,
                                binding: layout.binding as _,
                            };
                            Some((pso::Stage::Fragment, loc, &mut self.state.resources_fs))
                        } else {
                            None
                        };

                        for (stage, loc, resources) in bind_vs.into_iter().chain(bind_fs) {
                            if sampler_base != sm_range.start {
                                debug_assert_eq!(sampler_base, sm_range.end);
                                resources.set_samplers(
                                    pipe_layout.res_overrides[&loc].sampler_id as usize,
                                    &data.samplers[sm_range.clone()],
                                    |index, sampler| {
                                        pre.issue(soft::RenderCommand::BindSampler { stage, index, sampler });
                                    },
                                );
                            }
                            if texture_base != tx_range.start {
                                debug_assert_eq!(texture_base, tx_range.end);
                                resources.set_textures(
                                    pipe_layout.res_overrides[&loc].texture_id as usize,
                                    &data.textures[tx_range.clone()],
                                    |index, texture| {
                                        pre.issue(soft::RenderCommand::BindTexture { stage, index, texture });
                                    },
                                );
                            }
                            if buffer_base != bf_range.start {
                                debug_assert_eq!(buffer_base, bf_range.end);
                                let buffers = &data.buffers[bf_range.clone()];
                                let start = pipe_layout.res_overrides[&loc].buffer_id as usize;
                                let mut dynamic_index = 0;
                                for (i, bref) in buffers.iter().enumerate() {
                                    let (buffer, offset) = match bref.base {
                                        Some((buffer, mut offset)) => {
                                            if bref.dynamic {
                                                offset += dynamic_offsets[dynamic_index];
                                                dynamic_index += 1;
                                            }
                                            if !resources.set_buffer(start + i, buffer, offset as _) {
                                                continue
                                            }
                                            (Some(buffer), offset)
                                        }
                                        None => (None, 0),
                                    };
                                    pre.issue(soft::RenderCommand::BindBuffer {
                                        stage,
                                        index: start + i,
                                        buffer,
                                        offset,
                                    });
                                }
                            }
                        }
                    }
                }
                native::DescriptorSet::ArgumentBuffer { ref raw, offset, stage_flags, .. } => {
                    if stage_flags.contains(pso::ShaderStageFlags::VERTEX) {
                        let loc = msl::ResourceBindingLocation {
                            stage: spirv::ExecutionModel::Vertex,
                            desc_set: (first_set + set_index) as _,
                            binding: 0,
                        };
                        let slot = pipe_layout.res_overrides[&loc].buffer_id;
                        if self.state.resources_vs.set_buffer(slot as _, BufferPtr(raw.as_ptr()), offset as _) {
                            pre.issue(soft::RenderCommand::BindBuffer {
                                stage: pso::Stage::Vertex,
                                index: slot as _,
                                buffer: Some(BufferPtr(raw.as_ptr())),
                                offset,
                            });
                        }
                    }
                    if stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                        let loc = msl::ResourceBindingLocation {
                            stage: spirv::ExecutionModel::Fragment,
                            desc_set: (first_set + set_index) as _,
                            binding: 0,
                        };
                        let slot = pipe_layout.res_overrides[&loc].buffer_id;
                        if self.state.resources_fs.set_buffer(slot as _, BufferPtr(raw.as_ptr()), offset as _) {
                            pre.issue(soft::RenderCommand::BindBuffer {
                                stage: pso::Stage::Fragment,
                                index: slot as _,
                                buffer: Some(BufferPtr(raw.as_ptr())),
                                offset,
                            });
                        }
                    }
                }
            }
        }
    }

    fn bind_compute_pipeline(&mut self, pipeline: &native::ComputePipeline) {
        self.state.compute_pso = Some(pipeline.raw.clone());
        self.state.work_group_size = pipeline.work_group_size;

        let command = soft::ComputeCommand::BindPipeline(&*pipeline.raw);
        self.inner
            .borrow_mut()
            .sink()
            .pre_compute()
            .issue(command);
    }

    fn bind_compute_descriptor_sets<'a, I, J>(
        &mut self,
        pipe_layout: &native::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<native::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<com::DescriptorSetOffset>,
    {
        use spirv_cross::{msl, spirv};

        let mut offset_iter = offsets.into_iter();
        let mut dynamic_offsets = SmallVec::<[u64; 16]>::new();
        let mut inner = self.inner.borrow_mut();
        let mut pre = inner.sink().pre_compute();

        for (set_index, desc_set) in sets.into_iter().enumerate() {
            let resources = &mut self.state.resources_cs;
            let res_override = &pipe_layout.res_overrides[&msl::ResourceBindingLocation {
                stage: spirv::ExecutionModel::GlCompute,
                desc_set: (first_set + set_index) as _,
                binding: 0,
            }];
            match *desc_set.borrow() {
                native::DescriptorSet::Emulated { ref pool, ref layouts, ref sampler_range, ref texture_range, ref buffer_range } => {
                    let data = pool.read();
                    let mut sampler_base = sampler_range.start as usize;
                    let mut texture_base = texture_range.start as usize;
                    let mut buffer_base = buffer_range.start as usize;

                    for layout in layouts.iter() {
                        let sm_range = sampler_base .. sampler_base + layout.count;
                        let tx_range = texture_base .. texture_base + layout.count;
                        let bf_range = buffer_base .. buffer_base + layout.count;
                        native::DescriptorPool::count_bindings(layout.ty, layout.count,
                            &mut sampler_base, &mut texture_base, &mut buffer_base);

                        if buffer_base != bf_range.start {
                            dynamic_offsets.clear();
                            for bref in &data.buffers[bf_range.clone()] {
                                if bref.base.is_some() && bref.dynamic {
                                    dynamic_offsets.push(*offset_iter
                                        .next()
                                        .expect("No dynamic offset provided!")
                                        .borrow() as u64
                                    );
                                }
                            }
                        }

                        if sampler_base != sm_range.start {
                            debug_assert_eq!(sampler_base, sm_range.end);
                            resources.set_samplers(
                                res_override.sampler_id as usize,
                                &data.samplers[sm_range],
                                |index, sampler| {
                                    pre.issue(soft::ComputeCommand::BindSampler { index, sampler });
                                },
                            );
                        }
                        if texture_base != tx_range.start {
                            debug_assert_eq!(texture_base, tx_range.end);
                            resources.set_textures(
                                res_override.texture_id as usize,
                                &data.textures[tx_range],
                                |index, texture| {
                                    pre.issue(soft::ComputeCommand::BindTexture { index, texture });
                                },
                            );
                        }
                        if buffer_base != bf_range.start {
                            debug_assert_eq!(buffer_base, bf_range.end);
                            let buffers = &data.buffers[bf_range];
                            let start = res_override.buffer_id as usize;
                            let mut dynamic_index = 0;
                            for (i, bref) in buffers.iter().enumerate() {
                                let (buffer, offset) = match bref.base {
                                    Some((buffer, mut offset)) => {
                                        if bref.dynamic {
                                            offset += dynamic_offsets[dynamic_index];
                                            dynamic_index += 1;
                                        }
                                        if !resources.set_buffer(start + i, buffer, offset as _) {
                                            continue
                                        }
                                        (Some(buffer), offset)
                                    }
                                    None => (None, 0),
                                };
                                pre.issue(soft::ComputeCommand::BindBuffer {
                                    index: start + i,
                                    buffer,
                                    offset,
                                });
                            }
                        }
                    }
                }
                native::DescriptorSet::ArgumentBuffer { ref raw, offset, stage_flags, .. } => {
                    if stage_flags.contains(pso::ShaderStageFlags::COMPUTE) {
                        let index = res_override.buffer_id as usize;
                        let buffer = BufferPtr(raw.as_ptr());
                        if resources.set_buffer(index, buffer, offset as _) {
                            pre.issue(soft::ComputeCommand::BindBuffer {
                                index,
                                buffer: Some(buffer),
                                offset,
                            });
                        }
                    }
                }
            }
        }
    }

    fn dispatch(&mut self, count: WorkGroupCount) {
        let init_commands = self.state.make_compute_commands();

        let command = soft::ComputeCommand::Dispatch {
            wg_size: self.state.work_group_size,
            wg_count: MTLSize {
                width: count[0] as _,
                height: count[1] as _,
                depth: count[2] as _,
            },
        };

        let mut inner = self.inner.borrow_mut();
        let sink = inner.sink();
        //TODO: re-use compute encoders
        sink.begin_compute_pass(PassDoor::Open, init_commands);
        sink.compute_commands(iter::once(command));
        sink.stop_encoding();
    }

    fn dispatch_indirect(&mut self, buffer: &native::Buffer, offset: buffer::Offset) {
        let init_commands = self.state.make_compute_commands();

        let command = soft::ComputeCommand::DispatchIndirect {
            wg_size: self.state.work_group_size,
            buffer: BufferPtr(buffer.raw.as_ptr()),
            offset,
        };

        let mut inner = self.inner.borrow_mut();
        let sink = inner.sink();
        //TODO: re-use compute encoders
        sink.begin_compute_pass(PassDoor::Open, init_commands);
        sink.compute_commands(iter::once(command));
        sink.stop_encoding();
    }

    fn copy_buffer<T>(
        &mut self,
        src: &native::Buffer,
        dst: &native::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::BufferCopy>,
    {
        let pipes = self.shared.service_pipes.lock();
        let compute_pipe = pipes.get_copy_buffer();
        let wg_size = MTLSize {
            width: compute_pipe.thread_execution_width(),
            height: 1,
            depth: 1,
        };

        let mut inner = self.inner.borrow_mut();
        let mut blit_commands = Vec::new();
        let mut compute_commands = vec![ //TODO: get rid of heap
            soft::ComputeCommand::BindPipeline(compute_pipe),
        ];

        for region in regions {
            let r = region.borrow();
            if r.size % WORD_SIZE as u64 == 0 {
                blit_commands.push(soft::BlitCommand::CopyBuffer {
                    src: BufferPtr(src.raw.as_ptr()),
                    dst: BufferPtr(dst.raw.as_ptr()),
                    region: r.clone(),
                });
            } else {
                // not natively supported, going through compute shader
                assert_eq!(0, r.size >> 32);

                let wg_count = MTLSize {
                    width: (r.size + wg_size.width - 1) / wg_size.width,
                    height: 1,
                    depth: 1,
                };

                compute_commands.push(soft::ComputeCommand::BindBuffer {
                    index: 0,
                    buffer: Some(BufferPtr(dst.raw.as_ptr())),
                    offset: r.dst,
                });
                compute_commands.push(soft::ComputeCommand::BindBuffer {
                    index: 1,
                    buffer: Some(BufferPtr(src.raw.as_ptr())),
                    offset: r.src,
                });
                compute_commands.push(soft::ComputeCommand::BindBufferData {
                    index: 2,
                    words: unsafe { slice::from_raw_parts(
                        &(r.size as u32) as *const u32,
                        mem::size_of::<u32>() / WORD_SIZE,
                    )},
                });
                compute_commands.push(soft::ComputeCommand::Dispatch {
                    wg_size,
                    wg_count,
                });
            }
        }

        let sink = inner.sink();
        if !blit_commands.is_empty() {
            sink.blit_commands(blit_commands.into_iter());
        }

        if compute_commands.len() > 1 { // first is bind PSO
            sink.begin_compute_pass(
                PassDoor::Closed { label: "copy_buffer" },
                compute_commands.into_iter(),
            );
        }
    }

    fn copy_image<T>(
        &mut self,
        src: &native::Image,
        _src_layout: Layout,
        dst: &native::Image,
        _dst_layout: Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageCopy>,
    {
        let CommandBufferInner {
            ref mut retained_textures,
            ref mut sink,
            ..
        } = *self.inner.borrow_mut();

        let new_src = if src.mtl_format == dst.mtl_format {
            &*src.raw
        } else {
            assert_eq!(src.format_desc.bits, dst.format_desc.bits);
            let tex = src.raw.new_texture_view(dst.mtl_format);
            retained_textures.push(tex);
            retained_textures.last().unwrap()
        };

        let commands = regions.into_iter().map(|region| {
            soft::BlitCommand::CopyImage {
                src: TexturePtr(new_src.as_ptr()),
                dst: TexturePtr(dst.raw.as_ptr()),
                region: region.borrow().clone(),
            }
        });
        sink.as_mut()
            .unwrap()
            .blit_commands(commands);
    }

    fn copy_buffer_to_image<T>(
        &mut self,
        src: &native::Buffer,
        dst: &native::Image,
        _dst_layout: Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::BufferImageCopy>,
    {
        // FIXME: layout
        let commands = regions.into_iter().map(|region| {
            soft::BlitCommand::CopyBufferToImage {
                src: BufferPtr(src.raw.as_ptr()),
                dst: TexturePtr(dst.raw.as_ptr()),
                dst_desc: dst.format_desc,
                region: region.borrow().clone(),
            }
        });
        self.inner
            .borrow_mut()
            .sink()
            .blit_commands(commands);
    }

    fn copy_image_to_buffer<T>(
        &mut self,
        src: &native::Image,
        _src_layout: Layout,
        dst: &native::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::BufferImageCopy>,
    {
        // FIXME: layout
        let commands = regions.into_iter().map(|region| {
            soft::BlitCommand::CopyImageToBuffer {
                src: TexturePtr(src.raw.as_ptr()),
                src_desc: src.format_desc,
                dst: BufferPtr(dst.raw.as_ptr()),
                region: region.borrow().clone(),
            }
        });
        self.inner
            .borrow_mut()
            .sink()
            .blit_commands(commands);
    }

    fn draw(
        &mut self,
        vertices: Range<VertexCount>,
        instances: Range<InstanceCount>,
    ) {
        if instances.start == instances.end {
            return
        }

        let command = soft::RenderCommand::Draw {
            primitive_type: self.state.primitive_type,
            vertices,
            instances,
        };
        self.inner
            .borrow_mut()
            .sink()
            .render_commands(iter::once(command));
    }

    fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    ) {
        if instances.start == instances.end {
            return
        }

        let command = soft::RenderCommand::DrawIndexed {
            primitive_type: self.state.primitive_type,
            index: self.state.index_buffer.expect("must bind index buffer"),
            indices,
            base_vertex,
            instances,
        };
        self.inner
            .borrow_mut()
            .sink()
            .render_commands(iter::once(command));
    }

    fn draw_indirect(
        &mut self,
        buffer: &native::Buffer,
        offset: buffer::Offset,
        count: DrawCount,
        stride: u32,
    ) {
        assert_eq!(offset % WORD_ALIGNMENT, 0);
        assert_eq!(stride % WORD_ALIGNMENT as u32, 0);

        let commands = (0 .. count)
            .map(|i| soft::RenderCommand::DrawIndirect {
                primitive_type: self.state.primitive_type,
                buffer: BufferPtr(buffer.raw.as_ptr()),
                offset: offset + (i * stride) as buffer::Offset,
            });

        self.inner
            .borrow_mut()
            .sink()
            .render_commands(commands);
    }

    fn draw_indexed_indirect(
        &mut self,
        buffer: &native::Buffer,
        offset: buffer::Offset,
        count: DrawCount,
        stride: u32,
    ) {
        assert_eq!(offset % WORD_ALIGNMENT, 0);
        assert_eq!(stride % WORD_ALIGNMENT as u32, 0);

        let commands = (0 .. count)
            .map(|i| soft::RenderCommand::DrawIndexedIndirect {
                primitive_type: self.state.primitive_type,
                index: self.state.index_buffer.expect("must bind index buffer"),
                buffer: BufferPtr(buffer.raw.as_ptr()),
                offset: offset + (i * stride) as buffer::Offset,
            });

        self.inner
            .borrow_mut()
            .sink()
            .render_commands(commands);
    }

    fn begin_query(
        &mut self,
        _query: Query<Backend>,
        _flags: QueryControl,
    ) {
        unimplemented!()
    }

    fn end_query(
        &mut self,
        _query: Query<Backend>,
    ) {
        unimplemented!()
    }

    fn reset_query_pool(
        &mut self,
        _pool: &(),
        _queries: Range<QueryId>,
    ) {
        unimplemented!()
    }

    fn write_timestamp(
        &mut self,
        _: pso::PipelineStage,
        _: Query<Backend>,
    ) {
        // nothing to do, timestamps are unsupported on Metal
    }

    fn push_graphics_constants(
        &mut self,
        _layout: &native::PipelineLayout,
        stages: pso::ShaderStageFlags,
        offset: u32,
        constants: &[u32],
    ) {
        self.state.update_push_constants(offset, constants);
        let id = self.shared.push_constants_buffer_id;

        if stages.intersects(pso::ShaderStageFlags::GRAPHICS) {
            let mut inner = self.inner.borrow_mut();
            let mut pre = inner.sink().pre_render();
            // Note: the whole range is re-uploaded, which may be inefficient
            if stages.contains(pso::ShaderStageFlags::VERTEX) {
                pre.issue(self.state.push_vs_constants(id));
            }
            if stages.contains(pso::ShaderStageFlags::FRAGMENT) {
                pre.issue(self.state.push_ps_constants(id));
            }
        }
    }

    fn push_compute_constants(
        &mut self,
        _layout: &native::PipelineLayout,
        offset: u32,
        constants: &[u32],
    ) {
        self.state.update_push_constants(offset, constants);
        let id = self.shared.push_constants_buffer_id;

        // Note: the whole range is re-uploaded, which may be inefficient
        self.inner
            .borrow_mut()
            .sink()
            .pre_compute()
            .issue(self.state.push_cs_constants(id));
    }

    fn execute_commands<I>(
        &mut self,
        _buffers: I,
    ) where
        I: IntoIterator,
        I::Item: Borrow<CommandBuffer>
    {
        unimplemented!()
    }
}
