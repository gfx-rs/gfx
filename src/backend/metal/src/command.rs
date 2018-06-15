use {AutoreleasePool, Backend, Shared, validate_line_width};
use {conversions as conv, native, soft, window};
use internal::{BlitVertex, Channel, ClearKey, ClearVertex};

use std::borrow::Borrow;
use std::cell::RefCell;
use std::ops::{Deref, Range};
use std::sync::{Arc, Mutex};
use std::{iter, mem};
use std::slice;

use hal::{buffer, command as com, error, memory, pool, pso};
use hal::{DrawCount, FrameImage, VertexCount, VertexOffset, InstanceCount, IndexCount, WorkGroupCount};
use hal::backend::FastHashMap;
use hal::format::{Aspects, Format, FormatDesc};
use hal::image::{Extent, Filter, Layout, SubresourceRange};
use hal::pass::{AttachmentLoadOp, AttachmentOps};
use hal::query::{Query, QueryControl, QueryId};
use hal::queue::{RawCommandQueue, RawSubmission};
use hal::range::RangeArg;

use foreign_types::ForeignType;
use metal::{self, MTLViewport, MTLScissorRect, MTLPrimitiveType, MTLIndexType, MTLSize, CaptureManager};
use cocoa::foundation::{NSUInteger, NSInteger, NSRange};
use block::{ConcreteBlock};
use smallvec::SmallVec;


const WORD_ALIGNMENT: u64 = 4;
/// Enable an optimization to have multi-layered render passed
/// with clear operations set up to implement our `clear_image`
/// Note: currently doesn't work, needs a repro case for Apple
const CLEAR_IMAGE_ARRAY: bool = false;

pub struct QueueInner {
    raw: metal::CommandQueue,
    reserve: Range<usize>,
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
    pub(crate) fn new(device: &metal::DeviceRef, pool_size: usize) -> Self {
        QueueInner {
            raw: device.new_command_queue_with_max_command_buffer_count(pool_size as u64),
            reserve: 0 .. pool_size,
        }
    }

    /// Spawns a command buffer from a virtual pool.
    pub(crate) fn spawn(&mut self) -> (metal::CommandBuffer, Token) {
        let _pool = AutoreleasePool::new();
        self.reserve.start += 1;
        let cmd_buf = self.raw
            .new_command_buffer_with_unretained_references()
            .to_owned();
        (cmd_buf, Token { active: true })
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
        let (cmd_buf, token) = queue.lock().unwrap().spawn();
        cmd_buf.set_label("empty");
        cmd_buf.commit();
        cmd_buf.wait_until_completed();
        queue.lock().unwrap().release(token);
    }
}

type CommandBufferInnerPtr = Arc<RefCell<CommandBufferInner>>;

pub struct CommandPool {
    pub(crate) shared: Arc<Shared>,
    pub(crate) managed: Option<Vec<CommandBufferInnerPtr>>,
}

unsafe impl Send for CommandPool {}
unsafe impl Sync for CommandPool {}

#[derive(Clone)]
pub struct CommandBuffer {
    inner: CommandBufferInnerPtr,
    shared: Arc<Shared>,
    state: State,
}

#[derive(Clone)]
struct State {
    viewport: Option<MTLViewport>,
    scissors: Option<MTLScissorRect>,
    blend_color: Option<pso::ColorValue>,
    render_pso: Option<(metal::RenderPipelineState, native::VertexBufferMap, Vec<Option<Format>>)>,
    /// A flag to handle edge cases of Vulkan binding inheritance:
    /// we don't want to consider the current PSO bound for a new pass if it's not compatible.
    render_pso_is_compatible: bool,
    compute_pso: Option<metal::ComputePipelineState>,
    work_group_size: MTLSize,
    primitive_type: MTLPrimitiveType,
    resources_vs: StageResources,
    resources_fs: StageResources,
    resources_cs: StageResources,
    index_buffer: Option<IndexBuffer>,
    rasterizer_state: Option<native::RasterizerState>,
    pipeline_depth_stencil: Option<(pso::DepthStencilDesc, metal::DepthStencilState)>,
    dynamic_depth_stencil_desc: Option<metal::DepthStencilDescriptor>,
    dynamic_depth_stencil_state: Option<metal::DepthStencilState>,
    stencil: native::StencilState<pso::StencilValue>,
    push_constants: Vec<u32>,
    vertex_buffers: Vec<Option<(metal::Buffer, u64)>>,
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

    fn clamp_scissor(&self, sr: MTLScissorRect) -> MTLScissorRect {
        let ex = self.framebuffer_inner.extent;
        // sometimes there is not even an active render pass at this point
        let x = sr.x.min(ex.width.max(1) as u64 - 1);
        let y = sr.y.min(ex.height.max(1) as u64 - 1);
        //TODO: handle the zero scissor size sensibly
        MTLScissorRect {
            x,
            y,
            width: ((sr.x + sr.width).min(ex.width as u64) - x).max(1),
            height: ((sr.y + sr.height).min(ex.height as u64) - y).max(1),
        }
    }

    fn make_render_commands(&self, aspects: Aspects) -> Vec<soft::RenderCommand> {
        // TODO: re-use storage
        let mut commands = Vec::new();
        // Apply previously bound values for this command buffer
        commands.extend(self.viewport.map(soft::RenderCommand::SetViewport));
        if let Some(sr) = self.scissors {
            let clamped = self.clamp_scissor(sr);
            commands.push(soft::RenderCommand::SetScissor(clamped));
        }
        if aspects.contains(Aspects::COLOR) {
            commands.extend(self.blend_color.map(soft::RenderCommand::SetBlendColor));
        }
        if aspects.contains(Aspects::DEPTH) {
            commands.push(soft::RenderCommand::SetDepthBias(
                self.rasterizer_state.clone().map(|r| r.depth_bias).unwrap_or_default()
            ));
        }
        if self.render_pso_is_compatible {
            let rast = self.rasterizer_state.clone();
            commands.extend(self.render_pso.as_ref().map(|&(ref pso, _, _)| {
                soft::RenderCommand::BindPipeline(pso.clone(), rast)
            }));
        }

        let com = if let Some((_, ref static_state)) = self.pipeline_depth_stencil {
            Some(static_state.clone())
        } else if let Some(ref dynamic_state) = self.dynamic_depth_stencil_state {
            Some(dynamic_state.clone())
        } else {
            None
        };
        if aspects.intersects(Aspects::DEPTH | Aspects::STENCIL) {
            commands.extend(com.map(soft::RenderCommand::SetDepthStencilDesc));
        }

        let stages = [pso::Stage::Vertex, pso::Stage::Fragment];
        for (&stage, resources) in stages.iter().zip(&[&self.resources_vs, &self.resources_fs]) {
            commands.extend(resources.buffers.iter().enumerate().filter_map(|(i, resource)| {
                resource.clone().map(|(buffer, offset)| {
                    soft::RenderCommand::BindBuffer {
                        stage,
                        index: i as _,
                        buffer: Some(buffer),
                        offset,
                    }
                })
            }));
            commands.extend(resources.textures
                .iter()
                .cloned()
                .enumerate()
                .filter(|&(_, ref resource)| resource.is_some())
                .map(|(i, texture)| soft::RenderCommand::BindTexture {
                    stage,
                    index: i as _,
                    texture,
                })
            );
            commands.extend(resources.samplers
                .iter()
                .cloned()
                .enumerate()
                .filter(|&(_, ref resource)| resource.is_some())
                .map(|(i, sampler)| soft::RenderCommand::BindSampler {
                    stage,
                    index: i as _,
                    sampler,
                })
            );
            commands.extend(resources.push_constants_buffer_id
                .map(|id| soft::RenderCommand::BindBufferData {
                    stage,
                    index: id  as _,
                    bytes: soft::push_data(&self.push_constants),
                })
            );
        }
        commands
    }

    fn make_compute_commands(&self) -> Vec<soft::ComputeCommand> {
        let mut commands = Vec::new();

        commands.extend(self.compute_pso.clone().map(soft::ComputeCommand::BindPipeline));
        commands.extend(self.resources_cs.buffers.iter().enumerate().filter_map(|(i, resource)| {
            resource.clone().map(|(buffer, offset)| {
                soft::ComputeCommand::BindBuffer {
                    index: i as _,
                    buffer: Some(buffer),
                    offset,
                }
            })
        }));
        commands.extend(self.resources_cs.textures
            .iter()
            .cloned()
            .enumerate()
            .filter(|&(_, ref resource)| resource.is_some())
            .map(|(i, texture)| soft::ComputeCommand::BindTexture {
                index: i as _,
                texture,
            })
        );
        commands.extend(self.resources_cs.samplers
            .iter()
            .cloned()
            .enumerate()
            .filter(|&(_, ref resource)| resource.is_some())
            .map(|(i, sampler)| soft::ComputeCommand::BindSampler {
                index: i as _,
                sampler,
            })
        );
        commands.extend(self.resources_cs.push_constants_buffer_id
            .map(|id| soft::ComputeCommand::BindBufferData {
                index: id as _,
                bytes: soft::push_data(&self.push_constants),
            })
        );

        commands
    }
}

#[derive(Clone, Debug)]
struct StageResources {
    buffers: Vec<Option<(metal::Buffer, buffer::Offset)>>,
    textures: Vec<Option<native::ImageRoot>>,
    samplers: Vec<Option<metal::SamplerState>>,
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

    fn add_buffer(&mut self, slot: usize, buffer: &metal::BufferRef, offset: buffer::Offset) {
        while self.buffers.len() <= slot {
            self.buffers.push(None)
        }
        self.buffers[slot] = Some((buffer.to_owned(), offset));
    }

    fn add_textures(&mut self, start: usize, roots: &[Option<(native::ImageRoot, Layout)>]) {
        while self.textures.len() < start + roots.len() {
            self.textures.push(None)
        }
        for (out, root) in self.textures[start..].iter_mut().zip(roots.iter()) {
            *out = root.as_ref().map(|&(ref root, _)| root.clone());
        }
    }

    fn add_samplers(&mut self, start: usize, samplers: &[Option<metal::SamplerState>]) {
        while self.samplers.len() < start + samplers.len() {
            self.samplers.push(None)
        }
        for (out, sampler) in self.samplers[start..].iter_mut().zip(samplers.iter()) {
            *out = sampler.clone();
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

impl CommandSink {
    /// Issue provided (state-setting) commands only when there is already
    /// a render pass being actively encoded.
    /// The caller is expected to change the cached state accordingly, so these commands
    /// are going to be issued when a next pass starts, if not at this very moment.
    fn pre_render_commands<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = soft::RenderCommand>,
    {
        match *self {
            CommandSink::Immediate { encoder_state: EncoderState::Render(ref encoder), .. } => {
                for command in commands {
                    exec_render(encoder, &command);
                }
            }
            CommandSink::Deferred { ref mut passes, is_encoding: true } => {
                if let Some(&mut soft::Pass::Render { commands: ref mut list, .. }) = passes.last_mut() {
                    list.extend(commands);
                }
            }
            _ => {}
        }
    }

    /// Issue provided render commands, expecting that we are encoding a render pass.
    fn render_commands<I>(&mut self, commands: I)
    where
        I: Iterator<Item = soft::RenderCommand>,
    {
        match *self {
            CommandSink::Immediate { ref mut encoder_state, .. } => {
                match *encoder_state {
                    EncoderState::Render(ref encoder) => {
                        for command in commands {
                            exec_render(encoder, &command);
                        }
                    }
                    _ => panic!("Expected to be in render encoding state!")
                }
            }
            CommandSink::Deferred { ref mut passes, is_encoding } => {
                assert!(is_encoding);
                match passes.last_mut() {
                    Some(&mut soft::Pass::Render { commands: ref mut list, .. }) => {
                        list.extend(commands);
                    }
                    _ => panic!("Active pass is not a render pass")
                }
            }
        }
    }

    /// Issue provided blit commands. This function doesn't expect an active blit pass,
    /// it will automatically start one when needed.
    fn blit_commands<I>(&mut self, commands: I)
    where
        I: Iterator<Item = soft::BlitCommand>,
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
                    exec_blit(&encoder, &command);
                }

                *encoder_state = EncoderState::Blit(encoder);
            }
            CommandSink::Deferred { ref mut passes, .. } => {
                if let Some(&mut soft::Pass::Blit(ref mut list)) = passes.last_mut() {
                    list.extend(commands);
                    return;
                }
                passes.push(soft::Pass::Blit(commands.collect()));
            }
        }
    }

    /// Issue provided (state-setting) commands only when there is already
    /// a compute pass being actively encoded.
    /// The caller is expected to change the cached state accordingly, so these commands
    /// are going to be issued when a next pass starts, if not at this very moment.
    fn pre_compute_commands<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = soft::ComputeCommand>,
    {
        match *self {
            CommandSink::Immediate { encoder_state: EncoderState::Compute(ref encoder), .. } => {
                for command in commands {
                    exec_compute(encoder, &command);
                }
            }
            CommandSink::Deferred { ref mut passes, is_encoding: true } => {
                if let Some(&mut soft::Pass::Compute(ref mut list)) = passes.last_mut() {
                    list.extend(commands);
                }
            }
            _ => {}
        }
    }

    /// Issue provided compute commands, expecting that we are encoding a compute pass.
    fn compute_commands<I>(&mut self, commands: I)
    where
        I: Iterator<Item = soft::ComputeCommand>,
    {
        match *self {
            CommandSink::Immediate { ref mut encoder_state, .. } => {
                match *encoder_state {
                    EncoderState::Compute(ref encoder) => {
                        for command in commands {
                            exec_compute(encoder, &command);
                        }
                    }
                    _ => panic!("Expected to be in compute pass"),
                }
            }
            CommandSink::Deferred { ref mut passes, .. } => {
                if let Some(&mut soft::Pass::Compute(ref mut list)) = passes.last_mut() {
                    list.extend(commands);
                    return;
                }
                passes.push(soft::Pass::Compute(commands.collect()));
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

    fn quick_render_pass<I, J>(
        &mut self,
        descriptor: &metal::RenderPassDescriptorRef,
        frames: I,
        commands: J,
    ) where
        I: IntoIterator<Item = (usize, native::Frame)>,
        J: IntoIterator<Item = soft::RenderCommand>,
    {
        self.stop_encoding();

        match *self {
            CommandSink::Immediate { ref cmd_buffer, .. } => {
                let _ap = AutoreleasePool::new();
                resolve_frames(descriptor, frames);
                let encoder = cmd_buffer.new_render_command_encoder(descriptor);
                for command in commands {
                    exec_render(encoder, &command);
                }
                encoder.end_encoding();
            }
            CommandSink::Deferred { ref mut passes, .. } => {
                passes.push(soft::Pass::Render {
                    desc: descriptor.to_owned(),
                    frames: frames.into_iter().collect(),
                    commands: commands.into_iter().collect(),
                });
            }
        }
    }

    fn begin_render_pass<I>(
        &mut self,
        descriptor: metal::RenderPassDescriptor,
        frames: I,
        init_commands: Vec<soft::RenderCommand>,
    ) where
        I: Iterator<Item = (usize, native::Frame)>,
    {
        self.stop_encoding();

        match *self {
            CommandSink::Immediate { ref cmd_buffer, ref mut encoder_state, .. } => {
                let _ap = AutoreleasePool::new();
                resolve_frames(&descriptor, frames);
                let encoder = cmd_buffer.new_render_command_encoder(&descriptor);
                for command in init_commands {
                    exec_render(encoder, &command);
                }
                *encoder_state = EncoderState::Render(encoder.to_owned());
            }
            CommandSink::Deferred { ref mut passes, ref mut is_encoding } => {
                *is_encoding = true;
                passes.push(soft::Pass::Render {
                    desc: descriptor,
                    frames: frames.into_iter().collect(),
                    commands: init_commands,
                });
            }
        }
    }

    fn begin_compute_pass(
        &mut self,
        init_commands: Vec<soft::ComputeCommand>,
    ) {
        self.stop_encoding();

        match *self {
            CommandSink::Immediate { ref cmd_buffer, ref mut encoder_state, .. } => {
                let _ap = AutoreleasePool::new();
                let encoder = cmd_buffer.new_compute_command_encoder();
                for command in init_commands {
                    exec_compute(encoder, &command);
                }
                *encoder_state = EncoderState::Compute(encoder.to_owned());
            }
            CommandSink::Deferred { ref mut passes, ref mut is_encoding } => {
                *is_encoding = true;
                passes.push(soft::Pass::Compute(init_commands));
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct IndexBuffer {
    buffer: metal::Buffer,
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
        match self.sink.take() {
            Some(CommandSink::Immediate { token, mut encoder_state, .. }) => {
                encoder_state.end();
                shared.queue.lock().unwrap().release(token);
            }
            _ => {}
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

fn exec_render(encoder: &metal::RenderCommandEncoderRef, command: &soft::RenderCommand) {
    use soft::RenderCommand as Cmd;
    match *command {
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
        Cmd::SetDepthStencilDesc(ref depth_stencil_desc) => {
            encoder.set_depth_stencil_state(depth_stencil_desc);
        }
        Cmd::SetStencilReferenceValues(front, back) => {
            encoder.set_stencil_front_back_reference_value(front, back);
        }
        Cmd::BindBuffer { stage, index, ref buffer, offset } => {
            let buffer = buffer.as_ref().map(|x| x.as_ref());
            match stage {
                pso::Stage::Vertex =>
                    encoder.set_vertex_buffer(index as _, offset as _, buffer),
                pso::Stage::Fragment =>
                    encoder.set_fragment_buffer(index as _, offset as _, buffer),
                _ => unimplemented!()
            }
        }
        Cmd::BindBufferData { stage, ref bytes, index } => {
            match stage {
                pso::Stage::Vertex =>
                    encoder.set_vertex_bytes(index as _, bytes.len() as _, bytes.as_ptr() as _),
                pso::Stage::Fragment =>
                    encoder.set_fragment_bytes(index as _, bytes.len() as _, bytes.as_ptr() as _),
                _ => unimplemented!()
            }
        }
        Cmd::BindTexture { stage, index, ref texture } => {
            let guard;
            let texture = match texture {
                Some(ref root) => {
                    guard = root.resolve();
                    Some(&*guard)
                }
                None => None,
            };
            match stage {
                pso::Stage::Vertex =>
                    encoder.set_vertex_texture(index as _, texture),
                pso::Stage::Fragment =>
                    encoder.set_fragment_texture(index as _, texture),
                _ => unimplemented!()
            }
        }
        Cmd::BindSampler { stage, index, ref sampler } => {
            let sampler = sampler.as_ref().map(|x| x.as_ref());
            match stage {
                pso::Stage::Vertex =>
                    encoder.set_vertex_sampler_state(index as _, sampler),
                pso::Stage::Fragment =>
                    encoder.set_fragment_sampler_state(index as _, sampler),
                _ => unimplemented!()
            }
        }
        Cmd::BindPipeline(ref pipeline_state, ref rasterizer) => {
            encoder.set_render_pipeline_state(pipeline_state);
            if let Some(ref rasterizer_state) = *rasterizer {
                encoder.set_depth_clip_mode(rasterizer_state.depth_clip);
                let db = rasterizer_state.depth_bias;
                encoder.set_depth_bias(db.const_factor, db.slope_factor, db.clamp);
            }
        }
        Cmd::Draw { primitive_type, ref vertices, ref instances } =>  {
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
        Cmd::DrawIndexed { primitive_type, ref index, ref indices, base_vertex, ref instances } => {
            let index_size = match index.index_type {
                MTLIndexType::UInt16 => 2,
                MTLIndexType::UInt32 => 4,
            };
            let index_offset = index.offset + indices.start as buffer::Offset * index_size;
            // Metal requires `indexBufferOffset` alignment of 4
            if base_vertex == 0 && instances.start == 0 {
                //Note: for a strange reason, index alignment is not enforced here
                encoder.draw_indexed_primitives_instanced(
                    primitive_type,
                    (indices.end - indices.start) as NSUInteger,
                    index.index_type,
                    &index.buffer,
                    index_offset,
                    instances.end as NSUInteger,
                );
            } else {
                assert_eq!(index_offset % WORD_ALIGNMENT, 0);
                encoder.draw_indexed_primitives_instanced_base_instance(
                    primitive_type,
                    (indices.end - indices.start) as NSUInteger,
                    index.index_type,
                    &index.buffer,
                    index_offset,
                    (instances.end - instances.start) as NSUInteger,
                    base_vertex as NSInteger,
                    instances.start as NSUInteger,
                );
            }
        }
        Cmd::DrawIndirect { primitive_type, ref buffer, offset } => {
            encoder.draw_primitives_indirect(
                primitive_type,
                buffer,
                offset,
            );
        }
        Cmd::DrawIndexedIndirect { primitive_type, ref index, ref buffer, offset } => {
            encoder.draw_indexed_primitives_indirect(
                primitive_type,
                index.index_type,
                &index.buffer,
                index.offset,
                buffer,
                offset,
            );
        }
    }
}

pub(crate) fn exec_blit(encoder: &metal::BlitCommandEncoderRef, command: &soft::BlitCommand) {
    use soft::BlitCommand as Cmd;
    match *command {
        Cmd::CopyBuffer { ref src, ref dst, ref region } => {
            encoder.copy_from_buffer(
                src,
                region.src as NSUInteger,
                dst,
                region.dst as NSUInteger,
                region.size as NSUInteger
            );
        }
        Cmd::CopyImage { ref src, ref dst, ref region } => {
            let size = conv::map_extent(region.extent);
            let src_offset = conv::map_offset(region.src_offset);
            let dst_offset = conv::map_offset(region.dst_offset);
            let layers = region.src_subresource.layers.clone().zip(region.dst_subresource.layers.clone());
            for (src_layer, dst_layer) in layers {
                encoder.copy_from_texture(
                    &*src.resolve(),
                    src_layer as _,
                    region.src_subresource.level as _,
                    src_offset,
                    size,
                    &*dst.resolve(),
                    dst_layer as _,
                    region.dst_subresource.level as _,
                    dst_offset,
                );
            }
        }
        Cmd::CopyBufferToImage { ref src, ref dst, dst_desc, ref region } => {
            let extent = conv::map_extent(region.image_extent);
            let origin = conv::map_offset(region.image_offset);
            let (row_pitch, slice_pitch) = compute_pitches(&region, &dst_desc, &extent);
            let r = &region.image_layers;

            for layer in r.layers.clone() {
                let offset = region.buffer_offset + slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                encoder.copy_from_buffer_to_texture(
                    src,
                    offset as NSUInteger,
                    row_pitch as NSUInteger,
                    slice_pitch as NSUInteger,
                    extent,
                    &*dst.resolve(),
                    layer as NSUInteger,
                    r.level as NSUInteger,
                    origin,
                    metal::MTLBlitOption::empty(),
                );
            }
        }
        Cmd::CopyImageToBuffer { ref src, src_desc, ref dst, ref region } => {
            let extent = conv::map_extent(region.image_extent);
            let origin = conv::map_offset(region.image_offset);
            let (row_pitch, slice_pitch) = compute_pitches(&region, &src_desc, &extent);
            let r = &region.image_layers;

            for layer in r.layers.clone() {
                let offset = region.buffer_offset + slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                encoder.copy_from_texture_to_buffer(
                    &*src.resolve(),
                    layer as NSUInteger,
                    r.level as NSUInteger,
                    origin,
                    extent,
                    dst,
                    offset as NSUInteger,
                    row_pitch as NSUInteger,
                    slice_pitch as NSUInteger,
                    metal::MTLBlitOption::empty(),
                );
            }
        }
    }
}

fn exec_compute(encoder: &metal::ComputeCommandEncoderRef, command: &soft::ComputeCommand) {
    use soft::ComputeCommand as Cmd;
    match *command {
        Cmd::BindBuffer { index, ref buffer, offset } => {
            encoder.set_buffer(index as _, offset, buffer.as_ref().map(|x| x.as_ref()));
        }
        Cmd::BindBufferData { ref bytes, index } => {
            encoder.set_bytes(index as _, bytes.len() as _, bytes.as_ptr() as _);
        }
        Cmd::BindTexture { index, ref texture } => {
            let guard;
            let texture = match texture {
                Some(ref root) => {
                    guard = root.resolve();
                    Some(&*guard)
                }
                None => None,
            };
            encoder.set_texture(index as _, texture);
        }
        Cmd::BindSampler { index, ref sampler } => {
            encoder.set_sampler_state(index as _, sampler.as_ref().map(|x| x.as_ref()));
        }
        Cmd::BindPipeline(ref pipeline) => {
            encoder.set_compute_pipeline_state(pipeline);
        }
        Cmd::Dispatch { wg_size, wg_count } => {
            encoder.dispatch_thread_groups(wg_count, wg_size);
        }
        Cmd::DispatchIndirect { wg_size, ref buffer, offset } => {
            encoder.dispatch_thread_groups_indirect(buffer, offset, wg_size);
        }
    }
}

fn resolve_frames<I>(desc: &metal::RenderPassDescriptorRef, frames: I)
where
    I: IntoIterator,
    I::Item: Borrow<(usize, native::Frame)>,
{
    for f in frames {
        let (index, ref frame) = *f.borrow();
        let swapchain = frame.swapchain.read().unwrap();
        desc
            .color_attachments()
            .object_at(index as _)
            .unwrap()
            .set_texture(Some(&swapchain[frame.index]))
    }
}

fn record_commands(command_buf: &metal::CommandBufferRef, passes: &[soft::Pass]) {
    let _ap = AutoreleasePool::new(); // for encoder creation
    for pass in passes {
        match *pass {
            soft::Pass::Render { ref desc, ref frames, ref commands } => {
                resolve_frames(desc, frames);
                let encoder = command_buf.new_render_command_encoder(desc);
                for command in commands {
                    exec_render(&encoder, command);
                }
                encoder.end_encoding();
            }
            soft::Pass::Blit(ref commands) => {
                let encoder = command_buf.new_blit_command_encoder();
                for command in commands {
                    exec_blit(&encoder, command);
                }
                encoder.end_encoding();
            }
            soft::Pass::Compute(ref commands) => {
                let encoder = command_buf.new_compute_command_encoder();
                for command in commands {
                    exec_compute(&encoder, command);
                }
                encoder.end_encoding();
            }
        }
    }
}

unsafe impl Send for CommandBuffer {}
unsafe impl Sync for CommandBuffer {}

pub struct CommandQueue {
    shared: Arc<Shared>,
}

impl CommandQueue {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        CommandQueue {
            shared,
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
        debug!("submitting with fence {:?}", fence);
        // FIXME: wait for semaphores!

        // FIXME: multiple buffers signaling!
        let signal_block = if !submit.signal_semaphores.is_empty() {
            let semaphores_copy: Vec<_> = submit.signal_semaphores.iter().map(|semaphore| {
                semaphore.0
            }).collect();
            //Note: careful with those `ConcreteBlock::copy()` calls!
            Some(ConcreteBlock::new(move |_cb: *mut ()| -> () {
                for semaphore in semaphores_copy.iter() {
                    native::dispatch_semaphore_signal(*semaphore);
                }
            }).copy())
        } else {
            None
        };

        let queue = self.shared.queue.lock().unwrap();
        let (mut num_immediate, mut num_deferred) = (0, 0);

        for buffer in submit.cmd_buffers {
            let mut inner = buffer.borrow().inner.borrow_mut();
            let CommandBufferInner {
                ref sink,
                ref mut retained_buffers,
                ref mut retained_textures,
            } = *inner;

            let temp_cmd_buffer;
            let command_buffer: &metal::CommandBufferRef = match *sink {
                Some(CommandSink::Immediate { ref cmd_buffer, ref token, .. }) => {
                    num_immediate += 1;
                    trace!("\timmediate {:?}", token);
                    // schedule the retained buffers to release after the commands are done
                    if !retained_buffers.is_empty() || !retained_textures.is_empty() {
                        let free_buffers = mem::replace(retained_buffers, Vec::new());
                        let free_textures = mem::replace(retained_textures, Vec::new());
                        let release_block = ConcreteBlock::new(move |_cb: *mut ()| -> () {
                            // move and auto-release
                            let _ = free_buffers;
                            let _ = free_textures;
                        }).copy();
                        msg_send![*cmd_buffer, addCompletedHandler: release_block.deref() as *const _];
                    }
                    cmd_buffer
                }
                Some(CommandSink::Deferred { ref passes, .. }) => {
                    num_deferred += 1;
                    trace!("\tdeferred with {} passes", passes.len());
                    temp_cmd_buffer = queue.raw.new_command_buffer_with_unretained_references();
                    record_commands(&*temp_cmd_buffer, passes);
                    &*temp_cmd_buffer
                 }
                 _ => panic!("Command buffer not recorded for submission")
            };
            if let Some(ref signal_block) = signal_block {
                msg_send![command_buffer, addCompletedHandler: signal_block.deref() as *const _];
            }
            command_buffer.commit();
        }

        debug!("\t{} immediate, {} deferred command buffers", num_immediate, num_deferred);

        if let Some(ref fence) = fence {
            let command_buffer = queue.raw.new_command_buffer_with_unretained_references();
            let fence = Arc::clone(fence);
            let fence_block = ConcreteBlock::new(move |_cb: *mut ()| -> () {
                *fence.mutex.lock().unwrap() = true;
                fence.condvar.notify_all();
            }).copy();
            msg_send![command_buffer, addCompletedHandler: fence_block.deref() as *const _];
            command_buffer.commit();
        }
    }

    fn present<IS, S, IW>(&mut self, swapchains: IS, _wait_semaphores: IW) -> Result<(), ()>
    where
        IS: IntoIterator<Item = (S, FrameImage)>,
        S: Borrow<window::Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<native::Semaphore>,
    {
        for (swapchain, index) in swapchains {
            // TODO: wait for semaphores
            debug!("presenting frame {}", index);
            swapchain.borrow().present(index);
        }

        let shared_capture_manager = CaptureManager::shared();
        if shared_capture_manager.is_capturing() {
            shared_capture_manager.stop_capture();
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
        if let Some(ref mut managed) = self.managed {
            for cmd_buffer in managed {
                cmd_buffer
                    .borrow_mut()
                    .reset(&self.shared);
            }
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
                pipeline_depth_stencil: None,
                dynamic_depth_stencil_desc: None,
                dynamic_depth_stencil_state: None,
                stencil: native::StencilState::<pso::StencilValue> {
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
                    colors: Vec::new(),
                    depth_stencil: None,
                }
            },
        }).collect();

        if let Some(ref mut managed) = self.managed {
            managed.extend(buffers.iter().map(|buf| buf.inner.clone()));
        }
        buffers
    }

    /// Free command buffers which are allocated from this pool.
    unsafe fn free(&mut self, mut buffers: Vec<CommandBuffer>) {
        use hal::command::RawCommandBuffer;
        for buf in &mut buffers {
            buf.reset(true);
        }
        let managed = match self.managed {
            Some(ref mut vec) => vec,
            None => return,
        };
        for cmd_buf in buffers {
            match managed.iter_mut().position(|b| Arc::ptr_eq(b, &cmd_buf.inner)) {
                Some(index) => {
                    managed.swap_remove(index);
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
    fn set_viewport(&mut self, vp: &pso::Viewport) -> soft::RenderCommand {
        let viewport = MTLViewport {
            originX: vp.rect.x as _,
            originY: vp.rect.y as _,
            width: vp.rect.w as _,
            height: vp.rect.h as _,
            znear: vp.depth.start as _,
            zfar: if self.shared.disabilities.broken_viewport_near_depth {
                (vp.depth.end - vp.depth.start) as _
            } else {
                vp.depth.end as _
            },
        };
        self.state.viewport = Some(viewport);
        soft::RenderCommand::SetViewport(viewport)
    }

    fn set_scissor(&mut self, rect: &pso::Rect) -> soft::RenderCommand {
        let scissor = MTLScissorRect {
            x: rect.x as _,
            y: rect.y as _,
            width: rect.w as _,
            height: rect.h as _,
        };
        self.state.scissors = Some(scissor);
        let clamped = self.state.clamp_scissor(scissor);
        soft::RenderCommand::SetScissor(clamped)
    }

    fn set_blend_color(&mut self, color: &pso::ColorValue) -> soft::RenderCommand {
        self.state.blend_color = Some(*color);
        soft::RenderCommand::SetBlendColor(*color)
    }

    fn push_vs_constants(&mut self) -> soft::RenderCommand {
        let id = self.shared.push_constants_buffer_id;
        self.state.resources_vs.push_constants_buffer_id = Some(id);
        soft::RenderCommand::BindBufferData {
            stage: pso::Stage::Vertex,
            index: id as _,
            bytes: soft::push_data(&self.state.push_constants),
        }
    }

    fn push_ps_constants(&mut self) -> soft::RenderCommand {
        let id = self.shared.push_constants_buffer_id;
        self.state.resources_fs.push_constants_buffer_id = Some(id);
        soft::RenderCommand::BindBufferData {
            stage: pso::Stage::Fragment,
            index: id as _,
            bytes: soft::push_data(&self.state.push_constants),
        }
    }

    fn push_cs_constants(&mut self) -> soft::ComputeCommand {
        let id = self.shared.push_constants_buffer_id;
        self.state.resources_cs.push_constants_buffer_id = Some(id);
        soft::ComputeCommand::BindBufferData {
            index: id as _,
            bytes: soft::push_data(&self.state.push_constants),
        }
    }

    fn update_push_constants(
        &mut self,
        offset: u32,
        constants: &[u32],
    ) {
        assert_eq!(offset % WORD_ALIGNMENT as u32, 0);
        let offset = (offset  / WORD_ALIGNMENT as u32) as usize;
        let data = &mut self.state.push_constants;
        while data.len() < offset + constants.len() {
            data.push(0);
        }
        data[offset .. offset + constants.len()].copy_from_slice(constants);
    }

    fn set_depth_bias(&mut self, depth_bias: &pso::DepthBias) -> soft::RenderCommand {
        if let Some(ref mut r) = self.state.rasterizer_state {
            r.depth_bias = *depth_bias;
        } else {
            self.state.rasterizer_state = Some(native::RasterizerState {
                depth_bias: *depth_bias,
                ..Default::default()
            });
        }
        soft::RenderCommand::SetDepthBias(*depth_bias)
    }

    fn set_vertex_buffers(&mut self, commands: &mut Vec<soft::RenderCommand>) {
        let map = match self.state.render_pso {
            Some((_, ref map, _)) => map,
            None => return
        };

        let vs_buffers = &mut self.state.resources_vs.buffers;
        for (&(binding, extra_offset), vb) in map {
            let index = vb.binding as usize;
            while vs_buffers.len() <= index {
                vs_buffers.push(None)
            }
            let (buffer, offset) = match self.state.vertex_buffers.get(binding as usize) {
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

            commands.push(soft::RenderCommand::BindBuffer {
                stage: pso::Stage::Vertex,
                index,
                buffer: Some(buffer.clone()),
                offset,
            })
        }
    }

    fn set_depth_stencil_desc(
        &mut self,
        depth_stencil_desc: &pso::DepthStencilDesc,
        depth_stencil_raw: &metal::DepthStencilState,
    ) -> soft::RenderCommand {
        self.state.pipeline_depth_stencil = Some((depth_stencil_desc.clone(), depth_stencil_raw.clone()));
        soft::RenderCommand::SetDepthStencilDesc(depth_stencil_raw.clone())
    }

    fn set_stencil_reference_values(
        &mut self,
        front: pso::StencilValue,
        back: pso::StencilValue,
    ) -> soft::RenderCommand {
        self.state.stencil.front_reference = front;
        self.state.stencil.back_reference = back;
        soft::RenderCommand::SetStencilReferenceValues(front, back)
    }

    fn set_stencil_mask_values(
        &mut self,
        front_back_read_masks_to_update: Option<(pso::StencilValue, pso::StencilValue)>,
        front_back_write_masks_to_update: Option<(pso::StencilValue, pso::StencilValue)>,
        dynamic_depth_stencil_from_pipeline: Option<&metal::DepthStencilDescriptor>,
    ) -> Option<soft::RenderCommand> {
        if let Some((f, b)) = front_back_read_masks_to_update {
            self.state.stencil.front_read_mask = f;
            self.state.stencil.back_read_mask = b;
        }

        if let Some((f, b)) = front_back_write_masks_to_update {
            self.state.stencil.front_write_mask = f;
            self.state.stencil.back_write_mask = b;
        }

        if let Some(ds) = dynamic_depth_stencil_from_pipeline {
            self.state.dynamic_depth_stencil_desc = Some(ds.clone());
        }

        let dynamic_state = self.state.dynamic_depth_stencil_desc.as_ref().map(|desc| {
            let f_owned;
            let front = match desc.front_face_stencil() {
                Some(f) => f,
                None => {
                    f_owned = metal::StencilDescriptor::new();
                    desc.set_front_face_stencil(Some(&f_owned));
                    &f_owned
                }
            };

            let b_owned;
            let back = match desc.back_face_stencil() {
                Some(b) => b,
                None => {
                    b_owned = metal::StencilDescriptor::new();
                    desc.set_front_face_stencil(Some(&b_owned));
                    &b_owned
                }
            };

            if let Some((fm, bm)) = front_back_read_masks_to_update {
                front.set_read_mask(fm);
                back.set_read_mask(bm);
            }

            if let Some((fm, bm)) = front_back_write_masks_to_update {
                front.set_write_mask(fm);
                back.set_write_mask(bm);
            }

            self.shared.device
                .lock()
                .unwrap()
                .new_depth_stencil_state(&desc)
        });

        self.state.dynamic_depth_stencil_state = dynamic_state.as_ref().map(|ds| ds.clone());

        dynamic_state.map(soft::RenderCommand::SetDepthStencilDesc)
    }
}

impl com::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self, flags: com::CommandBufferFlags, _info: com::CommandBufferInheritanceInfo<Backend>) {
        self.reset(false);
        //TODO: Implement secondary command buffers
        let sink = if flags.contains(com::CommandBufferFlags::ONE_TIME_SUBMIT) {
            let (cmd_buffer, token) = self.shared.queue.lock().unwrap().spawn();
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
        let pipes = self.shared.service_pipes
            .lock()
            .unwrap();
        let pso = pipes
            .get_fill_buffer()
            .to_owned();

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

        let commands = vec![
            soft::ComputeCommand::BindPipeline(pso),
            soft::ComputeCommand::BindBuffer {
                index: 0,
                buffer: Some(buffer.raw.clone()),
                offset: start,
            },
            soft::ComputeCommand::BindBufferData {
                index: 1,
                bytes: unsafe {
                    slice::from_raw_parts(
                        value_and_length.as_ptr() as _,
                        mem::size_of::<u32>() * value_and_length.len()
                    ).to_owned()
                },
            },
            soft::ComputeCommand::Dispatch {
                wg_size,
                wg_count,
            },
        ];

        inner.sink().begin_compute_pass(commands);
        inner.sink().stop_encoding();
    }

    fn update_buffer(
        &mut self,
        dst: &native::Buffer,
        offset: buffer::Offset,
        data: &[u8],
    ) {
        let mut inner = self.inner.borrow_mut();
        let src = self.shared.device
            .lock()
            .unwrap()
            .new_buffer_with_data(
                data.as_ptr() as _,
                data.len() as _,
                metal::MTLResourceOptions::CPUCacheModeWriteCombined,
            );
        inner.retained_buffers.push(src.clone());

        let command = soft::BlitCommand::CopyBuffer {
            src,
            dst: dst.raw.clone(),
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
        let CommandBufferInner {
            ref mut retained_textures,
            ref mut sink,
            ..
        } = *self.inner.borrow_mut();
        let clear_color = image.shader_channel.interpret(color);

        for subresource_range in subresource_ranges {
            let sub = subresource_range.borrow();

            let mut frame = None;
            let num_layers = (sub.layers.end - sub.layers.start) as u64;
            let layers = if CLEAR_IMAGE_ARRAY {
                0 .. 1
            } else {
                sub.layers.clone()
            };
            let texture = if CLEAR_IMAGE_ARRAY && sub.layers.start > 0 {
                let image_raw = image.root.resolve();
                // aliasing is necessary for bulk-clearing all layers starting with 0
                let tex = image_raw.new_texture_view_from_slice(
                    image.mtl_format,
                    image.mtl_type,
                    NSRange {
                        location: 0,
                        length: image_raw.mipmap_level_count(),
                    },
                    NSRange {
                        location: sub.layers.start as _,
                        length: num_layers,
                    },
                );
                retained_textures.push(tex);
                retained_textures.last().map(|tex| tex.as_ref())
            } else {
                match image.root {
                    native::ImageRoot::Texture(ref tex) => Some(tex.as_ref()),
                    native::ImageRoot::Frame(ref f) => {
                        frame = Some((0, f.clone()));
                        None
                    }
                }
            };

            for layer in layers {
                for level in sub.levels.clone() {
                    let descriptor = metal::RenderPassDescriptor::new();
                    if image.extent.depth > 1 {
                        assert_eq!(sub.layers.end, 1);
                        let depth = image.extent.at_level(level).depth as u64;
                        descriptor.set_render_target_array_length(depth);
                    } else if CLEAR_IMAGE_ARRAY {
                        descriptor.set_render_target_array_length(num_layers);
                    };

                    let clear_color_attachment = sub.aspects.contains(Aspects::COLOR);
                    if clear_color_attachment || image.format_desc.aspects.contains(Aspects::COLOR) {
                        let attachment = descriptor
                            .color_attachments()
                            .object_at(0)
                            .unwrap();
                        attachment.set_texture(texture);
                        attachment.set_level(level as _);
                        attachment.set_store_action(metal::MTLStoreAction::Store);
                        if !CLEAR_IMAGE_ARRAY {
                            attachment.set_slice(layer as _);
                        }
                        if clear_color_attachment {
                            attachment.set_load_action(metal::MTLLoadAction::Clear);
                            attachment.set_clear_color(clear_color.clone());
                        } else {
                            attachment.set_load_action(metal::MTLLoadAction::Load);
                        }
                    }

                    let clear_depth_attachment = sub.aspects.contains(Aspects::DEPTH);
                    if clear_depth_attachment || image.format_desc.aspects.contains(Aspects::DEPTH) {
                        let attachment = descriptor
                            .depth_attachment()
                            .unwrap();
                        attachment.set_texture(texture);
                        attachment.set_level(level as _);
                        attachment.set_store_action(metal::MTLStoreAction::Store);
                        if !CLEAR_IMAGE_ARRAY {
                            attachment.set_slice(layer as _);
                        }
                        if clear_depth_attachment {
                            attachment.set_load_action(metal::MTLLoadAction::Clear);
                            attachment.set_clear_depth(depth_stencil.depth as _);
                        } else {
                            attachment.set_load_action(metal::MTLLoadAction::Load);
                        }
                    }

                    let clear_stencil_attachment = sub.aspects.contains(Aspects::STENCIL);
                    if clear_stencil_attachment || image.format_desc.aspects.contains(Aspects::STENCIL) {
                        let attachment = descriptor
                            .stencil_attachment()
                            .unwrap();
                        attachment.set_texture(texture);
                        attachment.set_level(level as _);
                        attachment.set_store_action(metal::MTLStoreAction::Store);
                        if !CLEAR_IMAGE_ARRAY {
                            attachment.set_slice(layer as _);
                        }
                        if clear_stencil_attachment {
                            attachment.set_load_action(metal::MTLLoadAction::Clear);
                            attachment.set_clear_stencil(depth_stencil.stencil);
                        } else {
                            attachment.set_load_action(metal::MTLLoadAction::Load);
                        }
                    }

                    sink.as_mut()
                        .unwrap()
                        .quick_render_pass(descriptor, frame.clone(), None);
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
        let mut vertices = Vec::new();
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

        let mut commands = Vec::new();
        let mut vertex_is_dirty = true;

        //  issue a PSO+color switch and a draw for each requested clear
        let mut pipes = self.shared.service_pipes
            .lock()
            .unwrap();

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

            let aspects = match *clear.borrow() {
                com::AttachmentClear::Color { index, value } => {
                    let cat = &self.state.framebuffer_inner.colors[index];
                    //Note: technically we should be able to derive the Channel from the
                    // `value` variant, but this is blocked by the portability that is
                    // always passing the attachment clears as `ClearColor::Float` atm.
                    let raw_value = com::ClearColorRaw::from(value);
                    commands.push(soft::RenderCommand::BindBufferData {
                        stage: pso::Stage::Fragment,
                        index: 0,
                        bytes: unsafe {
                            slice::from_raw_parts(raw_value.float32.as_ptr() as *const u8, 16)
                        }.to_owned(),
                    });
                    key.target_index = Some((index as u8, cat.channel));
                    Aspects::COLOR
                }
                com::AttachmentClear::DepthStencil { depth, stencil } => {
                    let mut aspects = Aspects::empty();
                    if let Some(value) = depth {
                        for v in &mut vertices {
                            v.pos[2] = value;
                        }
                        vertex_is_dirty = true;
                        aspects |= Aspects::DEPTH;
                    }
                    if let Some(_) = stencil {
                        //TODO: soft::RenderCommand::SetStencilReference
                        aspects |= Aspects::STENCIL;
                    }
                    aspects
                }
            };

            if vertex_is_dirty {
                vertex_is_dirty = false;
                commands.push(soft::RenderCommand::BindBufferData {
                    stage: pso::Stage::Vertex,
                    index: 0,
                    bytes: unsafe {
                        slice::from_raw_parts(
                            vertices.as_ptr() as *const u8,
                            vertices.len() * mem::size_of::<ClearVertex>()
                        ).to_owned()
                    }
                });
            }
            let pso = pipes.get_clear_image(
                key,
                &self.shared.device
            ).to_owned();
            commands.push(soft::RenderCommand::BindPipeline(pso, None));

            if !aspects.contains(Aspects::COLOR) {
                commands.push(soft::RenderCommand::SetDepthStencilDesc(
                    pipes.get_depth_stencil(aspects).to_owned()
                ));
            }

            commands.push(soft::RenderCommand::Draw {
                primitive_type: MTLPrimitiveType::Triangle,
                vertices: 0 .. vertices.len() as _,
                instances: 0 .. 1,
            });
        }

        // reset all the affected states
        if let Some((ref pso, _, _)) = self.state.render_pso {
            if self.state.render_pso_is_compatible {
                commands.push(soft::RenderCommand::BindPipeline(
                    pso.clone(),
                    None,
                ));
            } else {
                warn!("Not restoring the current PSO after clear_attachments because it's not compatible");
            }
        }

        if let Some((_, ref raw)) = self.state.pipeline_depth_stencil {
            commands.push(soft::RenderCommand::SetDepthStencilDesc(raw.clone()));
        }

        if let Some(&Some((ref buffer, offset))) = self.state.resources_vs.buffers.first() {
            commands.push(soft::RenderCommand::BindBuffer {
                stage: pso::Stage::Vertex,
                index: 0,
                buffer: Some(buffer.clone()),
                offset,
            });
        }
        if let Some(&Some((ref buffer, offset))) = self.state.resources_fs.buffers.first() {
            commands.push(soft::RenderCommand::BindBuffer {
                stage: pso::Stage::Fragment,
                index: 0,
                buffer: Some(buffer.clone()),
                offset,
            });
        }

        self.inner
            .borrow_mut()
            .sink()
            .render_commands(commands.into_iter());
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
        let mut vertices = FastHashMap::default(); // a list of vertices per mipmap
        let mut frame = None;
        let dst_texture = match dst.root {
            native::ImageRoot::Texture(ref tex) => Some(tex.as_ref()),
            native::ImageRoot::Frame(ref f) => {
                frame = Some((0, f.clone()));
                None
            }
        };

        for region in regions {
            let r = region.borrow();

            // layer count must be equal in both subresources
            debug_assert_eq!(r.src_subresource.layers.len(), r.dst_subresource.layers.len());
            debug_assert_eq!(r.src_subresource.aspects, r.dst_subresource.aspects);
            debug_assert!(src.format_desc.aspects.contains(r.src_subresource.aspects));
            debug_assert!(dst.format_desc.aspects.contains(r.dst_subresource.aspects));

            let se = src.extent.at_level(r.src_subresource.level);
            let de = dst.extent.at_level(r.dst_subresource.level);
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

        let mut inner = self.inner.borrow_mut();
        // Note: we don't bother to restore any render states here, since we are currently
        // outside of a render pass, and the state will be reset automatically once
        // we enter the next pass.
        let mut pipes = self.shared.service_pipes
            .lock()
            .unwrap();
        let key = (dst.mtl_type, dst.mtl_format, src.format_desc.aspects, dst.shader_channel);

        let mut prelude = vec![
            soft::RenderCommand::BindPipeline(
                pipes
                    .get_blit_image(key, &self.shared.device)
                    .to_owned(),
                None,
            ),
            soft::RenderCommand::BindSampler {
                stage: pso::Stage::Fragment,
                index: 0,
                sampler: Some(pipes.get_sampler(filter).to_owned()),
            },
            soft::RenderCommand::BindTexture {
                stage: pso::Stage::Fragment,
                index: 0,
                texture: Some(src.root.clone())
            },
        ];

        if src.format_desc.aspects.intersects(Aspects::DEPTH | Aspects::STENCIL) {
            prelude.push(soft::RenderCommand::SetDepthStencilDesc(
                pipes.get_depth_stencil(src.format_desc.aspects).to_owned()
            ));
        }

        for ((aspects, level), list) in vertices {
            let ext = &dst.extent;

            let extra = [
                //Note: flipping Y coordinate of the destination here
                soft::RenderCommand::SetViewport(MTLViewport {
                    originX: 0.0,
                    originY: (ext.height >> level) as _,
                    width: (ext.width >> level) as _,
                    height: -((ext.height >> level) as f64),
                    znear: 0.0,
                    zfar: 1.0,
                }),
                soft::RenderCommand::SetScissor(MTLScissorRect {
                    x: 0,
                    y: 0,
                    width: (ext.width >> level) as _,
                    height: (ext.height >> level) as _,
                }),
                soft::RenderCommand::BindBufferData {
                    stage: pso::Stage::Vertex,
                    index: 0,
                    bytes: unsafe {
                        slice::from_raw_parts(
                            list.as_ptr() as *const u8,
                            list.len() * mem::size_of::<BlitVertex>()
                        ).to_owned()
                    }
                },
                soft::RenderCommand::Draw {
                    primitive_type: MTLPrimitiveType::Triangle,
                    vertices: 0 .. list.len() as _,
                    instances: 0 .. 1,
                },
            ];

            let descriptor = metal::RenderPassDescriptor::new();
            descriptor.set_render_target_array_length(ext.depth as _);
            if aspects.contains(Aspects::COLOR) {
                let attachment = descriptor
                    .color_attachments()
                    .object_at(0)
                    .unwrap();
                attachment.set_texture(dst_texture);
                attachment.set_level(level as _);
            }
            if aspects.contains(Aspects::DEPTH) {
                let attachment = descriptor
                    .depth_attachment()
                    .unwrap();
                attachment.set_texture(dst_texture);
                attachment.set_level(level as _);
            }
            if aspects.contains(Aspects::STENCIL) {
                let attachment = descriptor
                    .stencil_attachment()
                    .unwrap();
                attachment.set_texture(dst_texture);
                attachment.set_level(level as _);
            }

            let commands = prelude
                .iter()
                .chain(&extra)
                .cloned();

            inner.sink().quick_render_pass(descriptor, frame.clone(), commands);
        }
    }

    fn bind_index_buffer(&mut self, view: buffer::IndexBufferView<Backend>) {
        let buffer = view.buffer.raw.clone();
        let offset = view.offset;
        let index_type = conv::map_index_type(view.index_type);
        self.state.index_buffer = Some(IndexBuffer {
            buffer,
            offset,
            index_type,
        });
    }

    fn bind_vertex_buffers(&mut self, first_binding: u32, buffer_set: pso::VertexBufferSet<Backend>) {
        while self.state.vertex_buffers.len() < first_binding as usize + buffer_set.0.len() {
            self.state.vertex_buffers.push(None);
        }
        for (i, &(buffer, offset)) in buffer_set.0.iter().enumerate() {
            self.state.vertex_buffers[first_binding as usize + i] = Some((buffer.raw.clone(), buffer.range.start + offset));
        }

        let mut commands = Vec::new();
        self.set_vertex_buffers(&mut commands);

        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(commands);
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

        let com = self.set_viewport(vp);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(iter::once(com));
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

        let com = self.set_scissor(rect);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(iter::once(com));
    }

    fn set_blend_constants(&mut self, color: pso::ColorValue) {
        let com = self.set_blend_color(&color);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(iter::once(com));
    }

    fn set_depth_bounds(&mut self, _: Range<f32>) {
        warn!("Depth bounds test is not supported");
    }

    fn set_line_width(&mut self, width: f32) {
        validate_line_width(width);
    }

    fn set_depth_bias(&mut self, depth_bias: pso::DepthBias) {
        let com = self.set_depth_bias(&depth_bias);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(iter::once(com));
    }

    fn set_stencil_reference(&mut self, faces: pso::Face, value: pso::StencilValue) {
        assert!(!faces.is_empty());

        let (front, back) = match faces {
            pso::Face::FRONT => (value, self.state.stencil.back_reference),
            pso::Face::BACK => (self.state.stencil.front_reference, value),
            _ => (value, value),
        };

        let com = self.set_stencil_reference_values(front, back);

        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(iter::once(com));
    }

    fn set_stencil_read_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        assert!(!faces.is_empty());

        let (front, back) = match faces {
            pso::Face::FRONT => (value, self.state.stencil.back_read_mask),
            pso::Face::BACK => (self.state.stencil.front_read_mask, value),
            _ => (value, value),
        };

        let com = self.set_stencil_mask_values(Some((front, back)), None, None);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(com);
    }

    fn set_stencil_write_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        assert!(!faces.is_empty());

        let (front, back) = match faces {
            pso::Face::FRONT => (value, self.state.stencil.back_write_mask),
            pso::Face::BACK => (self.state.stencil.front_write_mask, value),
            _ => (value, value),
        };

        let com = self.set_stencil_mask_values(None, Some((front, back)), None);
        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(com);
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
        let _ap = AutoreleasePool::new();
        // FIXME: subpasses
        let descriptor: metal::RenderPassDescriptor = unsafe {
            msg_send![framebuffer.descriptor, copy]
        };
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
            Some((_, _, ref formats)) => formats.len() == render_pass.attachments.len() &&
                formats.iter().zip(&render_pass.attachments).all(|(f, at)| *f == at.format),
            _ => false
        };

        self.state.framebuffer_inner = framebuffer.inner.clone();
        let frames = framebuffer.inner.colors
            .iter()
            .enumerate()
            .filter_map(|(index, ref cat)| cat.frame.clone().map(|f| (index, f)));
        let init_commands = self.state.make_render_commands(full_aspects);
        inner
            .sink()
            .begin_render_pass(descriptor, frames, init_commands);
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
        let pipeline_state = pipeline.raw.to_owned();
        self.state.render_pso_is_compatible = true; //assume good intent :)
        self.state.render_pso = Some((
            pipeline_state.clone(),
            pipeline.vertex_buffer_map.clone(),
            pipeline.attachment_formats.clone(),
        ));
        self.state.rasterizer_state = pipeline.rasterizer_state.clone();
        self.state.primitive_type = pipeline.primitive_type;

        let mut commands = Vec::new();
        commands.push(
            soft::RenderCommand::BindPipeline(
                pipeline_state,
                pipeline.rasterizer_state.clone(),
            )
        );
        if let Some(ref vp) = pipeline.baked_states.viewport {
            commands.push(self.set_viewport(vp));
        }
        if let Some(ref rect) = pipeline.baked_states.scissor {
            commands.push(self.set_scissor(rect));
        }
        if let Some(ref color) = pipeline.baked_states.blend_color {
            commands.push(self.set_blend_color(color));
        }

        let ds = &pipeline.depth_stencil_state;
        if let Some(desc) = ds.depth_stencil_desc {
            let command = match ds.depth_stencil_static {
                Some(ref raw) => Some(self.set_depth_stencil_desc(&desc, raw)),
                None => {
                    let front_r = ds.stencil.front_read_mask.static_or(self.state.stencil.front_read_mask);
                    let back_r = ds.stencil.back_read_mask.static_or(self.state.stencil.back_read_mask);
                    let front_w = ds.stencil.front_write_mask.static_or(self.state.stencil.front_write_mask);
                    let back_w = ds.stencil.back_write_mask.static_or(self.state.stencil.back_write_mask);
                    self.set_stencil_mask_values(
                        Some((front_r, back_r)),
                        Some((front_w, back_w)),
                        ds.depth_stencil_desc_raw.as_ref(),
                    )
                }
            };

            commands.extend(command);

            // If static stencil reference values were provided, update them here
            // Otherwise, leave any dynamic stencil reference values bound
            let front_ref = ds.stencil.front_reference.static_or(self.state.stencil.front_reference);
            let back_ref = ds.stencil.back_reference.static_or(self.state.stencil.back_reference);
            if ds.stencil.front_reference.is_static() || ds.stencil.back_reference.is_static() {
                commands.push(self.set_stencil_reference_values(front_ref, back_ref));
            }
        }

        // re-bind vertex buffers
        self.set_vertex_buffers(&mut commands);

        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(commands);
    }

    fn bind_graphics_descriptor_sets<'a, I, J>(
        &mut self,
        layout: &native::PipelineLayout,
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

        let mut commands = Vec::new(); //TODO: re-use the storage
        let mut offset_iter = offsets.into_iter();

        for (set_index, desc_set) in sets.into_iter().enumerate() {
            match *desc_set.borrow() {
                native::DescriptorSet::Emulated(ref desc_inner) => {
                    use native::DescriptorSetBinding::*;
                    let set = desc_inner.lock().unwrap();
                    let bindings = set.bindings
                        .iter()
                        .enumerate()
                        .filter_map(|(binding, values)| values.as_ref().map(|v| (binding as u32, v)));

                    for (binding, values) in bindings {
                        let desc_layout = set.layout.iter().find(|x| x.binding == binding).unwrap();

                        let mut bind_stages = SmallVec::<[_; 2]>::new();
                        if desc_layout.stage_flags.contains(pso::ShaderStageFlags::VERTEX) {
                            let loc = msl::ResourceBindingLocation {
                                stage: spirv::ExecutionModel::Vertex,
                                desc_set: (first_set + set_index) as _,
                                binding: binding as _,
                            };
                            bind_stages.push((pso::Stage::Vertex, loc, &mut self.state.resources_vs));
                        }
                        if desc_layout.stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                            let loc = msl::ResourceBindingLocation {
                                stage: spirv::ExecutionModel::Fragment,
                                desc_set: (first_set + set_index) as _,
                                binding: binding as _,
                            };
                            bind_stages.push((pso::Stage::Fragment, loc, &mut self.state.resources_fs));
                        }

                        match values {
                            Sampler(ref samplers) => {
                                for &mut (stage, ref loc, ref mut resources) in &mut bind_stages {
                                    let start = layout.res_overrides[loc].sampler_id as usize;
                                    resources.add_samplers(start, samplers.as_slice());
                                    commands.extend(samplers.iter().cloned().enumerate().map(|(i, sampler)| {
                                        soft::RenderCommand::BindSampler {
                                            stage,
                                            index: start + i,
                                            sampler,
                                        }
                                    }));
                                }
                            }
                            Image(ref images) => {
                                for &mut (stage, ref loc, ref mut resources) in &mut bind_stages {
                                    let start = layout.res_overrides[loc].texture_id as usize;
                                    resources.add_textures(start, images.as_slice());
                                    commands.extend(images.iter().enumerate().map(|(i, texture)| {
                                        soft::RenderCommand::BindTexture {
                                            stage,
                                            index: start + i,
                                            texture: texture.as_ref().map(|&(ref root, _)| root.clone()),
                                        }
                                    }));
                                }
                            }
                            Combined(ref combos) => {
                                for &mut (stage, ref loc, ref mut resources) in &mut bind_stages {
                                    let start_tx = layout.res_overrides[loc].texture_id as usize;
                                    let start_sm = layout.res_overrides[loc].sampler_id as usize;
                                    for (i, (ref texture, ref sampler)) in combos.iter().cloned().enumerate() {
                                        resources.add_textures(start_tx + i, &[texture.clone()]);
                                        resources.add_samplers(start_sm + i, &[sampler.clone()]);
                                        commands.push(soft::RenderCommand::BindTexture {
                                            stage,
                                            index: start_tx + i,
                                            texture: texture.as_ref().map(|&(ref root, _)| root.clone()),
                                        });
                                        commands.push(soft::RenderCommand::BindSampler {
                                            stage,
                                            index: start_sm + i,
                                            sampler: sampler.clone(),
                                        });
                                    }
                                }
                            }
                            Buffer(ref buffers) => {
                                for (i, bref) in buffers.iter().enumerate() {
                                    let (buffer, offset) = match bref.base {
                                        Some((ref buffer, mut offset)) => {
                                            if bref.dynamic {
                                                offset += *offset_iter
                                                    .next()
                                                    .expect("No dynamic offset provided!")
                                                    .borrow() as u64;
                                            }
                                            (Some(buffer), offset)
                                        }
                                        None => (None, 0),
                                    };
                                    for &mut (stage, ref loc, ref mut resources) in &mut bind_stages {
                                        let start = layout.res_overrides[loc].buffer_id as usize;
                                        if let Some(buffer) = buffer {
                                            resources.add_buffer(start + i, buffer, offset as _);
                                        }
                                        commands.push(soft::RenderCommand::BindBuffer {
                                            stage,
                                            index: start + i,
                                            buffer: buffer.cloned(),
                                            offset,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                native::DescriptorSet::ArgumentBuffer { ref buffer, offset, stage_flags, .. } => {
                    if stage_flags.contains(pso::ShaderStageFlags::VERTEX) {
                        let loc = msl::ResourceBindingLocation {
                            stage: spirv::ExecutionModel::Vertex,
                            desc_set: (first_set + set_index) as _,
                            binding: 0,
                        };
                        let slot = layout.res_overrides[&loc].buffer_id;
                        self.state.resources_vs.add_buffer(slot as _, buffer, offset as _);
                        commands.push(soft::RenderCommand::BindBuffer {
                            stage: pso::Stage::Vertex,
                            index: slot as _,
                            buffer: Some(buffer.clone()),
                            offset,
                        });
                    }
                    if stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                        let loc = msl::ResourceBindingLocation {
                            stage: spirv::ExecutionModel::Fragment,
                            desc_set: (first_set + set_index) as _,
                            binding: 0,
                        };
                        let slot = layout.res_overrides[&loc].buffer_id;
                        self.state.resources_fs.add_buffer(slot as _, &buffer, offset as _);
                        commands.push(soft::RenderCommand::BindBuffer {
                            stage: pso::Stage::Fragment,
                            index: slot as _,
                            buffer: Some(buffer.clone()),
                            offset,
                        });
                    }
                }
            }
        }

        self.inner
            .borrow_mut()
            .sink()
            .pre_render_commands(commands);
    }

    fn bind_compute_pipeline(&mut self, pipeline: &native::ComputePipeline) {
        self.state.compute_pso = Some(pipeline.raw.clone());
        self.state.work_group_size = pipeline.work_group_size;

        let command = soft::ComputeCommand::BindPipeline(pipeline.raw.clone());

        self.inner
            .borrow_mut()
            .sink()
            .pre_compute_commands(iter::once(command));
    }

    fn bind_compute_descriptor_sets<'a, I, J>(
        &mut self,
        layout: &native::PipelineLayout,
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

        let mut commands = Vec::new();
        let mut offset_iter = offsets.into_iter();

        for (set_index, desc_set) in sets.into_iter().enumerate() {
            let resources = &mut self.state.resources_cs;
            let location_cs = msl::ResourceBindingLocation {
                stage: spirv::ExecutionModel::GlCompute,
                desc_set: (first_set + set_index) as _,
                binding: 0,
            };
            match *desc_set.borrow() {
                native::DescriptorSet::Emulated(ref desc_inner) => {
                    use native::DescriptorSetBinding::*;
                    let set = desc_inner.lock().unwrap();
                    let bindings = set.bindings
                        .iter()
                        .enumerate()
                        .filter_map(|(binding, values)| values.as_ref().map(|v| (binding as u32, v)));

                    for (binding, values) in bindings {
                        let desc_layout = set.layout.iter().find(|x| x.binding == binding).unwrap();

                        if desc_layout.stage_flags.contains(pso::ShaderStageFlags::COMPUTE) {
                            let res = &layout.res_overrides[&msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_cs
                            }];
                            match values {
                                Sampler(ref samplers) => {
                                    let start = res.sampler_id as usize;
                                    resources.add_samplers(start, samplers.as_slice());
                                    commands.extend(samplers.iter().cloned().enumerate().map(|(i, sampler)| {
                                        soft::ComputeCommand::BindSampler {
                                            index: start + i,
                                            sampler,
                                        }
                                    }));
                                }
                                Image(ref images) => {
                                    let start = res.texture_id as usize;
                                    resources.add_textures(start, images.as_slice());
                                    commands.extend(images.iter().enumerate().map(|(i, texture)| {
                                        soft::ComputeCommand::BindTexture {
                                            index: start + i,
                                            texture: texture.as_ref().map(|&(ref texture, _)| texture.clone()),
                                        }
                                    }));
                                }
                                Combined(ref combos) => {
                                    for (i, (ref texture, ref sampler)) in combos.iter().cloned().enumerate() {
                                        let id_tx = res.texture_id as usize + i;
                                        let id_sm = res.sampler_id as usize + i;
                                        resources.add_textures(id_tx, &[texture.clone()]);
                                        resources.add_samplers(id_sm, &[sampler.clone()]);
                                        commands.push(soft::ComputeCommand::BindTexture {
                                            index: id_tx,
                                            texture: texture.as_ref().map(|&(ref root, _)| root.clone()),
                                        });
                                        commands.push(soft::ComputeCommand::BindSampler {
                                            index: id_sm,
                                            sampler: sampler.clone(),
                                        });
                                    }
                                }
                                Buffer(ref buffers) => {
                                    let start = res.buffer_id as usize;
                                    for (i, bref) in buffers.iter().enumerate() {
                                        let (buffer, offset) = match bref.base {
                                            Some((ref buffer, mut offset)) => {
                                                if bref.dynamic {
                                                    offset += *offset_iter
                                                        .next()
                                                        .expect("No dynamic offset provided!")
                                                        .borrow() as u64;
                                                }
                                                resources.add_buffer(start + i, buffer.as_ref(), offset as _);
                                                (Some(buffer.clone()), offset)
                                            },
                                            None => (None, 0),
                                        };
                                        commands.push(soft::ComputeCommand::BindBuffer {
                                            index: start + i,
                                            buffer,
                                            offset,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                native::DescriptorSet::ArgumentBuffer { ref buffer, offset, stage_flags, .. } => {
                    if stage_flags.contains(pso::ShaderStageFlags::COMPUTE) {
                        let slot = layout.res_overrides[&location_cs].buffer_id;
                        resources.add_buffer(slot as _, buffer, offset as _);
                    }
                }
            }
        }

        self.inner
            .borrow_mut()
            .sink()
            .pre_compute_commands(commands);
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
        sink.begin_compute_pass(init_commands);
        sink.compute_commands(iter::once(command));
        sink.stop_encoding();
    }

    fn dispatch_indirect(&mut self, buffer: &native::Buffer, offset: buffer::Offset) {
        let init_commands = self.state.make_compute_commands();

        let command = soft::ComputeCommand::DispatchIndirect {
            wg_size: self.state.work_group_size,
            buffer: buffer.raw.clone(),
            offset,
        };

        let mut inner = self.inner.borrow_mut();
        let sink = inner.sink();
        //TODO: re-use compute encoders
        sink.begin_compute_pass(init_commands);
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
        let compute_pipe = self.shared.service_pipes
            .lock()
            .unwrap()
            .get_copy_buffer()
            .to_owned();
        let wg_size = MTLSize {
            width: compute_pipe.thread_execution_width(),
            height: 1,
            depth: 1,
        };

        let mut inner = self.inner.borrow_mut();
        let mut blit_commands = Vec::new();
        let mut compute_commands = vec![
            soft::ComputeCommand::BindPipeline(compute_pipe),
        ];

        for region in regions {
            let r = region.borrow();
            if r.size % 4 == 0 {
                blit_commands.push(soft::BlitCommand::CopyBuffer {
                    src: src.raw.clone(),
                    dst: dst.raw.clone(),
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
                    buffer: Some(dst.raw.clone()),
                    offset: r.dst,
                });
                compute_commands.push(soft::ComputeCommand::BindBuffer {
                    index: 1,
                    buffer: Some(src.raw.clone()),
                    offset: r.src,
                });
                compute_commands.push(soft::ComputeCommand::BindBufferData {
                    index: 2,
                    bytes: unsafe {
                        slice::from_raw_parts(
                            &(r.size as u32) as *const u32 as _,
                            mem::size_of::<u32>()
                        ).to_owned()
                    }
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
            sink.begin_compute_pass(compute_commands);
            sink.stop_encoding();
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
            src.root.clone()
        } else {
            assert_eq!(src.format_desc.bits, dst.format_desc.bits);
            let tex = src.root.resolve().new_texture_view(dst.mtl_format);
            retained_textures.push(tex.clone());
            native::ImageRoot::Texture(tex)
        };

        let commands = regions.into_iter().map(|region| {
            soft::BlitCommand::CopyImage {
                src: new_src.clone(),
                dst: dst.root.clone(),
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
                src: src.raw.clone(),
                dst: dst.root.clone(),
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
                src: src.root.clone(),
                src_desc: src.format_desc,
                dst: dst.raw.clone(),
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
            index: self.state.index_buffer.clone().expect("must bind index buffer"),
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
                buffer: buffer.raw.clone(),
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
                index: self.state.index_buffer.clone().expect("must bind index buffer"),
                buffer: buffer.raw.clone(),
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
        self.update_push_constants(offset, constants);

        if stages.intersects(pso::ShaderStageFlags::GRAPHICS) {
            // Note: it's a waste to heap allocate the bytes here in case
            // of no active render pass.
            // Note: the whole range is re-uploaded, which may be inefficient
            let com_vs = if stages.contains(pso::ShaderStageFlags::VERTEX) {
                Some(self.push_vs_constants())
            } else {
                None
            };
            let com_ps = if stages.contains(pso::ShaderStageFlags::FRAGMENT) {
                Some(self.push_ps_constants())
            } else {
                None
            };
            let commands = com_vs.into_iter().chain(com_ps);

            self.inner
                .borrow_mut()
                .sink()
                .pre_render_commands(commands);
        }
    }

    fn push_compute_constants(
        &mut self,
        _layout: &native::PipelineLayout,
        offset: u32,
        constants: &[u32],
    ) {
        self.update_push_constants(offset, constants);

        // Note: it's a waste to heap allocate the bytes here in case
        // of no active render pass.
        // Note: the whole range is re-uploaded, which may be inefficient
        let command = self.push_cs_constants();

        self.inner
            .borrow_mut()
            .sink()
            .pre_compute_commands(iter::once(command));
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
