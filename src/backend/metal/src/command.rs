use {AutoreleasePool, Backend};
use {native, window};
use internal::{BlitVertex, ServicePipes};

use std::borrow::{Borrow, BorrowMut};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ops::{Deref, Range};
use std::sync::{Arc, Mutex};
use std::{iter, mem};

use hal::{buffer, command as com, error, memory, pool, pso};
use hal::{VertexCount, VertexOffset, InstanceCount, IndexCount, WorkGroupCount};
use hal::format::{Aspects, FormatDesc};
use hal::image::{Filter, Layout, SubresourceRange};
use hal::query::{Query, QueryControl, QueryId};
use hal::queue::{RawCommandQueue, RawSubmission};
use hal::range::RangeArg;

use metal::{self, MTLViewport, MTLScissorRect, MTLPrimitiveType, MTLClearColor, MTLIndexType, MTLSize, CaptureManager};
use cocoa::foundation::{NSUInteger, NSInteger};
use block::{ConcreteBlock};
use {conversions as conv, soft};

use smallvec::SmallVec;


pub(crate) struct QueueInner {
    queue: metal::CommandQueue,
    reserve: Range<usize>,
}

#[derive(Default)]
pub struct QueuePool {
    queues: Vec<QueueInner>,
}

pub struct CommandBufferScope<'a> {
    inner: &'a metal::CommandBufferRef,
    _pool: AutoreleasePool,
}

impl<'a> Deref for CommandBufferScope<'a> {
    type Target = metal::CommandBufferRef;
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl QueuePool {
    fn find_queue(&mut self, device: &metal::DeviceRef) -> usize {
        self.queues
            .iter()
            .position(|q| q.reserve.start != q.reserve.end)
            .unwrap_or_else(|| {
                let queue = QueueInner {
                    queue: device.new_command_queue(),
                    reserve: 0 .. 64, //TODO
                };
                self.queues.push(queue);
                self.queues.len() - 1
            })
    }

    pub fn borrow_command_buffer(
        &mut self, device: &metal::DeviceRef
    ) -> CommandBufferScope {
        let pool = AutoreleasePool::new();
        let id = self.find_queue(device);
        let inner = self.queues[id].queue
            .new_command_buffer_with_unretained_references();
        CommandBufferScope {
            inner,
            _pool: pool,
        }
    }

    pub fn make_command_buffer(
        &mut self, device: &metal::DeviceRef
    ) -> (usize, metal::CommandBuffer) {
        let _pool = AutoreleasePool::new();
        let id = self.find_queue(device);
        self.queues[id].reserve.start += 1;
        let cmd_buffer = self.queues[id].queue
            .new_command_buffer_with_unretained_references()
            .to_owned();
        (id, cmd_buffer)
    }

    pub fn release_command_buffer(&mut self, index: usize) {
        self.queues[index].reserve.start -= 1;
    }
}

pub struct Shared {
    device: metal::Device,
    pub(crate) queue_pool: Mutex<QueuePool>,
    service_pipes: Mutex<ServicePipes>,
}

unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Shared {
    pub fn new(device: metal::Device) -> Self {
        Shared {
            queue_pool: Mutex::new(QueuePool::default()),
            service_pipes: Mutex::new(ServicePipes::new(&device)),
            device,
        }
    }
}


type CommandBufferInnerPtr = Arc<UnsafeCell<CommandBufferInner>>;

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
    render_pso: Option<metal::RenderPipelineState>,
    compute_pso: Option<metal::ComputePipelineState>,
    work_group_size: MTLSize,
    primitive_type: MTLPrimitiveType,
    resources_vs: StageResources,
    resources_fs: StageResources,
    resources_cs: StageResources,
    index_buffer: Option<IndexBuffer>,
    attribute_buffer_index: usize,
    depth_stencil_state: Option<metal::DepthStencilState>,
}

impl State {
    fn reset_resources(&mut self) {
        self.resources_vs.clear();
        self.resources_fs.clear();
        self.resources_cs.clear();
    }

    fn make_render_commands(&self) -> Vec<soft::RenderCommand> {
        // TODO: re-use storage
        let mut commands = Vec::new();
        // Apply previously bound values for this command buffer
        commands.extend(self.viewport.map(soft::RenderCommand::SetViewport));
        commands.extend(self.scissors.map(soft::RenderCommand::SetScissor));
        commands.extend(self.blend_color.map(soft::RenderCommand::SetBlendColor));
        let depth_stencil = self.depth_stencil_state.clone();
        commands.extend(self.render_pso.clone().map(|pipeline| {
            soft::RenderCommand::BindPipeline(pipeline, depth_stencil)
        }));
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

        commands
    }
}

#[derive(Clone, Debug)]
struct StageResources {
    buffers: Vec<Option<(metal::Buffer, buffer::Offset)>>,
    textures: Vec<Option<metal::Texture>>,
    samplers: Vec<Option<metal::SamplerState>>,
}

impl StageResources {
    fn new() -> Self {
        StageResources {
            buffers: Vec::new(),
            textures: Vec::new(),
            samplers: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.buffers.clear();
        self.textures.clear();
        self.samplers.clear();
    }

    fn add_buffer(&mut self, slot: usize, buffer: &metal::BufferRef, offset: buffer::Offset) {
        while self.buffers.len() <= slot {
            self.buffers.push(None)
        }
        self.buffers[slot] = Some((buffer.to_owned(), offset));
    }

    fn add_textures(&mut self, start: usize, textures: &[Option<(metal::Texture, Layout)>]) {
        while self.textures.len() < start + textures.len() {
            self.textures.push(None)
        }
        for (out, entry) in self.textures[start..].iter_mut().zip(textures.iter()) {
            *out = entry.as_ref().map(|&(ref texture, _)| texture.clone());
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

enum CommandSink {
    Immediate {
        cmd_buffer: metal::CommandBuffer,
        queue_index: usize,
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
                if let Some(&mut soft::Pass::Render(_, ref mut list)) = passes.last_mut() {
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
                    Some(&mut soft::Pass::Render(_, ref mut list)) => {
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
                match mem::replace(encoder_state, EncoderState::None)  {
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
            CommandSink::Deferred { ref mut is_encoding, .. } => {
                *is_encoding = false;
            }
        }
    }

    fn begin_render_pass(
        &mut self,
        init_commands: Vec<soft::RenderCommand>,
        descriptor: metal::RenderPassDescriptor,
    ) {
        self.stop_encoding();

        match *self {
            CommandSink::Immediate { ref cmd_buffer, ref mut encoder_state, .. } => {
                let _ap = AutoreleasePool::new();
                let encoder = cmd_buffer.new_render_command_encoder(&descriptor);
                for command in init_commands {
                    exec_render(encoder, &command);
                }
                *encoder_state = EncoderState::Render(encoder.to_owned());
            }
            CommandSink::Deferred { ref mut passes, ref mut is_encoding } => {
                *is_encoding = true;
                passes.push(soft::Pass::Render(descriptor, init_commands));
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
            Some(CommandSink::Immediate { queue_index, .. }) => {
                shared.queue_pool
                    .lock()
                    .unwrap()
                    .release_command_buffer(queue_index);
            }
            _ => {}
        }
        self.retained_buffers.clear();
    }
}


enum EncoderState {
    None,
    Blit(metal::BlitCommandEncoder),
    Render(metal::RenderCommandEncoder),
    Compute(metal::ComputeCommandEncoder),
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
        Cmd::BindTexture { stage, index, ref texture } => {
            let texture = texture.as_ref().map(|x| x.as_ref());
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
        Cmd::BindPipeline(ref pipeline_state, ref depth_stencil) => {
            encoder.set_render_pipeline_state(pipeline_state);
            if let Some(ref depth_stencil_state) = *depth_stencil {
                encoder.set_depth_stencil_state(depth_stencil_state);
            }
        }
        Cmd::Draw { primitive_type, ref vertices, ref instances } =>  {
            encoder.draw_primitives_instanced_base_instance(
                primitive_type,
                vertices.start as NSUInteger,
                (vertices.end - vertices.start) as NSUInteger,
                (instances.end - instances.start) as NSUInteger,
                instances.start as NSUInteger
            );
        }
        Cmd::DrawIndexed { ref index, primitive_type, ref indices, base_vertex, ref instances } => {
            let index_offset = indices.start as buffer::Offset * match index.index_type {
                MTLIndexType::UInt16 => 2,
                MTLIndexType::UInt32 => 4,
            };
            // Metal requires `indexBufferOffset` alignment of 4
            assert_eq!((index_offset + index.offset) & 3, 0);
            encoder.draw_indexed_primitives_instanced(
                primitive_type,
                (indices.end - indices.start) as NSUInteger,
                index.index_type,
                &index.buffer,
                (index_offset + index.offset) as NSUInteger,
                (instances.end - instances.start) as NSUInteger,
                base_vertex as NSInteger,
                instances.start as NSUInteger,
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
                    src,
                    src_layer as _,
                    region.src_subresource.level as _,
                    src_offset,
                    size,
                    dst,
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
                    dst,
                    layer as NSUInteger,
                    r.level as NSUInteger,
                    origin,
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
                    src,
                    layer as NSUInteger,
                    r.level as NSUInteger,
                    origin,
                    extent,
                    dst,
                    offset as NSUInteger,
                    row_pitch as NSUInteger,
                    slice_pitch as NSUInteger
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
        Cmd::BindTexture { index, ref texture } => {
            encoder.set_texture(index as _, texture.as_ref().map(|x| x.as_ref()));
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

fn record_commands(command_buf: &metal::CommandBufferRef, passes: &[soft::Pass]) {
    let _ap = AutoreleasePool::new(); // for encoder creation
    for pass in passes {
        match *pass {
            soft::Pass::Render(ref desc, ref list) => {
                let encoder = command_buf.new_render_command_encoder(desc);
                for command in list {
                    exec_render(&encoder, command);
                }
                encoder.end_encoding();
            }
            soft::Pass::Blit(ref list) => {
                let encoder = command_buf.new_blit_command_encoder();
                for command in list {
                    exec_blit(&encoder, command);
                }
                encoder.end_encoding();
            }
            soft::Pass::Compute(ref list) => {
                let encoder = command_buf.new_compute_command_encoder();
                for command in list {
                    exec_compute(&encoder, command);
                }
                encoder.end_encoding();
            }
        }
    }
}

unsafe impl Send for CommandBuffer {}
unsafe impl Sync for CommandBuffer {}

impl RawCommandQueue<Backend> for Arc<Shared> {
    unsafe fn submit_raw<IC>(
        &mut self, submit: RawSubmission<Backend, IC>, fence: Option<&native::Fence>
    )
    where
        IC: IntoIterator,
        IC::Item: Borrow<CommandBuffer>,
    {
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

        let mut pool = self.queue_pool.lock().unwrap();

        for buffer in submit.cmd_buffers {
            let inner = &mut *buffer.borrow().inner.get();
            let temp_cmd_buffer;
            let command_buffer: &metal::CommandBufferRef = match inner.sink {
                Some(CommandSink::Immediate { ref cmd_buffer, .. }) => {
                    // schedule the retained buffers to release after the commands are done
                    if !inner.retained_buffers.is_empty() {
                        let retained_buffers = mem::replace(&mut inner.retained_buffers, Vec::new());
                        let release_block = ConcreteBlock::new(move |_cb: *mut ()| -> () {
                            let _ = retained_buffers; // move and auto-release
                        }).copy();
                        let cb_ref: &metal::CommandBufferRef = cmd_buffer;
                        msg_send![cb_ref, addCompletedHandler: release_block.deref() as *const _];
                    }
                    cmd_buffer
                }
                Some(CommandSink::Deferred { ref passes, .. }) => {
                    temp_cmd_buffer = pool.borrow_command_buffer(&self.device);
                    record_commands(&*temp_cmd_buffer, passes);
                    &*temp_cmd_buffer
                 }
                 None => panic!("Command buffer not recorded for submission")
            };
            if let Some(ref signal_block) = signal_block {
                msg_send![command_buffer, addCompletedHandler: signal_block.deref() as *const _];
            }
            command_buffer.commit();
        }

        if let Some(ref fence) = fence {
            let command_buffer = pool.borrow_command_buffer(&self.device);
            let value_ptr = fence.0.clone();
            let fence_block = ConcreteBlock::new(move |_cb: *mut ()| -> () {
                *value_ptr.lock().unwrap() = true;
            }).copy();
            msg_send![command_buffer, addCompletedHandler: fence_block.deref() as *const _];
            command_buffer.commit();
        }
    }

    fn present<IS, IW>(&mut self, swapchains: IS, _wait_semaphores: IW)
    where
        IS: IntoIterator,
        IS::Item: BorrowMut<window::Swapchain>,
        IW: IntoIterator,
        IW::Item: Borrow<native::Semaphore>,
    {
        for mut swapchain in swapchains {
            // TODO: wait for semaphores
            let swapchain = swapchain.borrow_mut();
            let (surface, io_surface) = swapchain.present();
            unsafe {
                let render_layer_borrow = surface.render_layer.lock().unwrap();
                let render_layer = *render_layer_borrow;
                msg_send![render_layer, setContents: io_surface.obj];
            }
        }

        if cfg!(debug_assertions) || cfg!(feature = "metal_default_capture_scope") {
            let shared_capture_manager = CaptureManager::shared();
            if let Some(default_capture_scope) = shared_capture_manager.default_capture_scope() {
                default_capture_scope.end_scope();
                default_capture_scope.begin_scope();
            }
        }
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        let mut pool = self.queue_pool.lock().unwrap();
        let cmd_buffer = pool.borrow_command_buffer(&self.device);
        cmd_buffer.commit();
        cmd_buffer.wait_until_completed();
        Ok(())
    }
}

impl pool::RawCommandPool<Backend> for CommandPool {
    fn reset(&mut self) {
        if let Some(ref mut managed) = self.managed {
            for cmd_buffer in managed {
                unsafe { &mut *cmd_buffer.get() }
                    .reset(&self.shared);
            }
        }
    }

    fn allocate(
        &mut self, num: usize, _level: com::RawLevel
    ) -> Vec<CommandBuffer> {
        //TODO: Implement secondary buffers
        let buffers: Vec<_> = (0..num).map(|_| CommandBuffer {
            inner: Arc::new(UnsafeCell::new(CommandBufferInner {
                sink: None,
                retained_buffers: Vec::new(),
            })),
            shared: self.shared.clone(),
            state: State {
                viewport: None,
                scissors: None,
                blend_color: None,
                render_pso: None,
                compute_pso: None,
                work_group_size: MTLSize { width: 0, height: 0, depth: 0 },
                primitive_type: MTLPrimitiveType::Point,
                resources_vs: StageResources::new(),
                resources_fs: StageResources::new(),
                resources_cs: StageResources::new(),
                index_buffer: None,
                attribute_buffer_index: 0,
                depth_stencil_state: None,
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
            //TODO: what else here?
            let inner_ptr = cmd_buf.inner.get();
            match managed.iter_mut().position(|b| inner_ptr == b.get()) {
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

impl CommandBuffer {
    fn sink(&mut self) -> &mut CommandSink {
        let inner = unsafe { &mut *self.inner.get() };
        inner.sink.as_mut().unwrap()
    }

    fn set_viewport(&mut self, vp: &pso::Viewport) -> soft::RenderCommand {
        let viewport = MTLViewport {
            originX: vp.rect.x as _,
            originY: vp.rect.y as _,
            width: vp.rect.w as _,
            height: vp.rect.h as _,
            znear: vp.depth.start as _,
            zfar: vp.depth.end as _,
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
        soft::RenderCommand::SetScissor(scissor)
    }

    fn set_blend_color(&mut self, color: &pso::ColorValue) -> soft::RenderCommand {
        self.state.blend_color = Some(*color);
        soft::RenderCommand::SetBlendColor(*color)
    }
}

impl com::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self, flags: com::CommandBufferFlags, _info: com::CommandBufferInheritanceInfo<Backend>) {
        //TODO: Implement secondary command buffers
        let sink = if flags.contains(com::CommandBufferFlags::ONE_TIME_SUBMIT) {
            let (queue_index, cmd_buffer) = self.shared.queue_pool
                .lock()
                .unwrap()
                .make_command_buffer(&self.shared.device);
            CommandSink::Immediate {
                cmd_buffer,
                queue_index,
                encoder_state: EncoderState::None,
            }
        } else {
            CommandSink::Deferred {
                passes: Vec::new(),
                is_encoding: false,
            }
        };

        unsafe { &mut *self.inner.get() }.sink = Some(sink);
        self.state.reset_resources();
    }

    fn finish(&mut self) {
        self.sink().stop_encoding();
    }

    fn reset(&mut self, _release_resources: bool) {
        self.state.reset_resources();
        unsafe { &mut *self.inner.get() }
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
        R: RangeArg<u64>,
    {
        let inner = unsafe { &mut *self.inner.get() };
        let sink = inner.sink.as_mut().unwrap();

        let pipes = self.shared.service_pipes
            .lock()
            .unwrap();

        let pso = pipes
            .get_fill_buffer()
            .to_owned();

        // TODO: Reuse buffer allocation
        let fill = self.shared.device.new_buffer_with_data(
            &data as *const u32 as *const _,
            mem::size_of::<u32>() as _,
            metal::MTLResourceOptions::StorageModeShared,
        );

        inner.retained_buffers.push(fill.clone());

        let start = *range.start().unwrap();
        assert_eq!(start & 3, 0);

        let end = match range.end() {
            Some(e) => {
                assert_eq!(e & 3, 0);
                *e
            },
            None =>  (buffer.raw.length() - buffer.offset) & !3,
        };

        const WORD_ALIGNMENT: u64 = 4;

        // TODO: Choose appropriate value and possibly fill multiple elements per thread in shader
        // For now we simply write one element per thread, one thread per threadgroup
        let threadgroups = (end - start) / WORD_ALIGNMENT;
        let threads = 1;
        let wg_count = MTLSize {
            width: threadgroups,
            height: 1,
            depth: 1,
        };
        let wg_size = MTLSize {
            width: threads,
            height: 1,
            depth: 1,
        };

        let commands = vec![
            soft::ComputeCommand::BindPipeline(pso),
            soft::ComputeCommand::BindBuffer {
                index: 0,
                buffer: Some(buffer.raw.clone()),
                offset: buffer.offset + start,
            },
            soft::ComputeCommand::BindBuffer {
                index: 1,
                buffer: Some(fill),
                offset: 0,
            },
            soft::ComputeCommand::Dispatch {
                wg_count,
                wg_size,
            },
        ];

        sink.begin_compute_pass(commands);
        sink.stop_encoding();
    }

    fn update_buffer(
        &mut self,
        dst: &native::Buffer,
        offset: buffer::Offset,
        data: &[u8],
    ) {
        let inner = unsafe { &mut *self.inner.get() };
        let src = self.shared.device.new_buffer_with_data(
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
        inner.sink
            .as_mut()
            .unwrap()
            .blit_commands(iter::once(command));
    }

    fn clear_color_image_raw(
        &mut self,
        _image: &native::Image,
        _layout: Layout,
        _range: SubresourceRange,
        _value: com::ClearColorRaw,
    ) {
        unimplemented!()
    }

    fn clear_depth_stencil_image_raw(
        &mut self,
        _image: &native::Image,
        _layout: Layout,
        _range: SubresourceRange,
        _value: com::ClearDepthStencilRaw,
    ) {
        unimplemented!()
    }

    fn clear_attachments<T, U>(
        &mut self,
        _clears: T,
        _rects: U,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::AttachmentClear>,
        U: IntoIterator,
        U::Item: Borrow<pso::Rect>,
    {
        unimplemented!()
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
        use hal::image::{Extent, Offset};

        fn range_size(r: &Range<Offset>) -> Option<Extent> {
            let dx = r.end.x - r.start.x;
            let dy = r.end.y - r.start.y;
            let dz = r.end.z - r.start.z;
            if dx >= 0 && dy >= 0 && dz >= 0 {
                Some(Extent {
                    width: dx as _,
                    height: dy as _,
                    depth: dz as _,
                })
            } else {
                None
            }
        }

        #[inline]
        fn is_offset_positive(o: &Offset) -> bool {
            o.x >= 0 && o.y >= 0 && o.z >= 0
        }

        #[inline]
        fn has_depth_stencil_format(i: &native::Image) -> bool {
            i.format_desc.aspects.contains(Aspects::DEPTH | Aspects::STENCIL)
        }

        // We check if either of the two images has a combined depth/stencil format
        let has_ds = has_depth_stencil_format(&src) || has_depth_stencil_format(&dst);
        let mut blit_commands = Vec::new();
        let mut blit_vertices = HashMap::new(); // a list of vertices per mipmap

        for region in regions {
            let r = region.borrow();

            // layer count must be equal in both subresources
            debug_assert_eq!(r.src_subresource.layers.len(), r.dst_subresource.layers.len());
            // aspect flags
            // check formats compatibility
            if src.format_desc.bits != dst.format_desc.bits {
                error!("Formats {:?} and {:?} are not compatible, ignoring blit_image", src.mtl_format, dst.mtl_format);
                continue;
            }
            // enforce aspect flag restrictions
            debug_assert_ne!(r.src_subresource.aspects.contains(Aspects::COLOR), r.src_subresource.aspects.contains(Aspects::DEPTH | Aspects::STENCIL));
            debug_assert_ne!(r.dst_subresource.aspects.contains(Aspects::COLOR), r.dst_subresource.aspects.contains(Aspects::DEPTH | Aspects::STENCIL));
            debug_assert_eq!(r.src_subresource.aspects, r.dst_subresource.aspects);
            // check that we're only copying aspects actually in the image
            debug_assert!(src.format_desc.aspects.contains(r.src_subresource.aspects));

            let only_one_depth_stencil = {
                let has_depth = r.src_subresource.aspects.contains(Aspects::DEPTH);
                let has_stencil = r.src_subresource.aspects.contains(Aspects::STENCIL);
                has_depth ^ has_stencil
            };

            let src_size = range_size(&r.src_bounds);
            let dst_size = range_size(&r.dst_bounds);
            // In the case that the image format is a combined Depth / Stencil format,
            // and we are only copying one of the aspects, we use the shader even if the regions
            // are the same size
            if src_size == dst_size && src_size.is_some() && !(has_ds && only_one_depth_stencil) {
                debug_assert!(is_offset_positive(&r.src_bounds.start));
                debug_assert!(is_offset_positive(&r.dst_bounds.start));

                blit_commands.push(soft::BlitCommand::CopyImage {
                    src: src.raw.clone(),
                    dst: src.raw.clone(),
                    region: com::ImageCopy {
                        src_subresource: r.src_subresource.clone(),
                        src_offset: r.src_bounds.start,
                        dst_subresource: r.dst_subresource.clone(),
                        dst_offset: r.dst_bounds.start,
                        extent: src_size.unwrap(),
                    },
                });
            } else {
                // Fall back to shader-based blitting
                let se = &src.extent;
                let de = &dst.extent;
                //TODO: support 3D textures
                debug_assert_eq!(se.depth, 1);
                debug_assert_eq!(de.depth, 1);

                let layers = r.src_subresource.layers.clone().zip(r.dst_subresource.layers.clone());
                let list = blit_vertices
                    .entry(r.dst_subresource.level)
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
                                dst_layer as f32,
                                1.0,
                            ],
                        });
                    }
                }
            }
        }

        self.sink().blit_commands(blit_commands.into_iter());

        if !blit_vertices.is_empty() {
            let inner = unsafe { &mut *self.inner.get() };
            let sink = inner.sink.as_mut().unwrap();

            // Note: we don't bother to restore any render states here, since we are currently
            // outside of a render pass, and the state will be reset automatically once
            // we enter the next pass.
            let mut pipes = self.shared.service_pipes
                .lock()
                .unwrap();
            let pso = pipes
                .get_blit_image(dst.mtl_type, dst.mtl_format, &self.shared.device)
                .to_owned();
            let sampler = pipes.get_sampler(filter);
            let ext = &dst.extent;

            let prelude = [
                soft::RenderCommand::BindPipeline(pso, None),
                soft::RenderCommand::SetViewport(MTLViewport {
                    originX: 0.0,
                    originY: 0.0,
                    width: ext.width as _,
                    height: ext.height as _,
                    znear: 0.0,
                    zfar: 1.0,
                }),
                soft::RenderCommand::SetScissor(MTLScissorRect {
                    x: 0,
                    y: 0,
                    width: ext.width as _,
                    height: ext.height as _,
                }),
                soft::RenderCommand::BindSampler {
                    stage: pso::Stage::Fragment,
                    index: 0,
                    sampler: Some(sampler),
                },
                soft::RenderCommand::BindTexture {
                    stage: pso::Stage::Fragment,
                    index: 0,
                    texture: Some(src.raw.clone())
                },
            ];

            for (level, list) in blit_vertices {
                //Note: we might want to re-use the buffer allocation, but that
                // requires proper re-cycling based on the command buffer fences.
                let vertex_buffer = self.shared.device.new_buffer_with_data(
                    list.as_ptr() as *const _,
                    (list.len() * mem::size_of::<BlitVertex>()) as _,
                    metal::MTLResourceOptions::StorageModeShared,
                );
                inner.retained_buffers.push(vertex_buffer.clone());

                let mut commands = prelude.to_vec();
                commands.push(soft::RenderCommand::BindBuffer {
                    stage: pso::Stage::Vertex,
                    index: 0,
                    buffer: Some(vertex_buffer),
                    offset: 0,
                });
                commands.push(soft::RenderCommand::Draw {
                    primitive_type: MTLPrimitiveType::Triangle,
                    vertices: 0 .. list.len() as _,
                    instances: 0 .. 1,
                });

                let descriptor = metal::RenderPassDescriptor::new().to_owned();
                descriptor.set_render_target_array_length(ext.depth as _);
                {
                    let attachment = descriptor
                        .color_attachments()
                        .object_at(0)
                        .unwrap();
                    attachment.set_texture(Some(&dst.raw));
                    attachment.set_level(level as _);
                }

                sink.begin_render_pass(commands, descriptor);
                // no actual pass body - everything is in the init commands
            }

            sink.stop_encoding();
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
        let attribute_buffer_index = self.state.attribute_buffer_index + first_binding as usize;
        {
            let buffers = &mut self.state.resources_vs.buffers;
            while buffers.len() < attribute_buffer_index + buffer_set.0.len()    {
                buffers.push(None)
            }
            for (ref mut out, &(ref buffer, offset)) in buffers[attribute_buffer_index..].iter_mut().zip(buffer_set.0.iter()) {
                **out = Some((buffer.raw.clone(), offset));
            }
        }

        let commands = buffer_set.0.iter().enumerate().map(|(i, &(buffer, offset))| {
            soft::RenderCommand::BindBuffer {
                stage: pso::Stage::Vertex,
                index: attribute_buffer_index + i,
                buffer: Some(buffer.raw.clone()),
                offset,
            }
        });
        self.sink().pre_render_commands(commands);
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
        self.sink().pre_render_commands(iter::once(com));
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
        self.sink().pre_render_commands(iter::once(com));
    }

    fn set_stencil_reference(&mut self, _front: pso::StencilValue, _back: pso::StencilValue) {
        unimplemented!()
    }

    fn set_blend_constants(&mut self, color: pso::ColorValue) {
        let com = self.set_blend_color(&color);
        self.sink().pre_render_commands(iter::once(com));
    }

    fn set_depth_bounds(&mut self, _: Range<f32>) {
        warn!("Depth bounds test is not supported");
    }

    fn begin_render_pass_raw<T>(
        &mut self,
        render_pass: &native::RenderPass,
        frame_buffer: &native::FrameBuffer,
        _render_area: pso::Rect,
        clear_values: T,
        _first_subpass: com::SubpassContents,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ClearValueRaw>,
    {
        let descriptor = unsafe {
            // FIXME: subpasses
            let pass_descriptor: metal::RenderPassDescriptor = msg_send![frame_buffer.0, copy];

            for (i, value) in clear_values.into_iter().enumerate() {
                let value = *value.borrow();
                if i < render_pass.num_colors {
                    let color_desc = pass_descriptor.color_attachments().object_at(i).expect("too many clear values");
                    let mtl_color = MTLClearColor::new(
                        value.color.float32[0] as f64,
                        value.color.float32[1] as f64,
                        value.color.float32[2] as f64,
                        value.color.float32[3] as f64,
                    );
                    color_desc.set_clear_color(mtl_color);
                } else {
                    let depth_desc = pass_descriptor.depth_attachment().expect("no depth attachment");
                    depth_desc.set_clear_depth(value.depth_stencil.depth as f64);
                }
            }

            pass_descriptor
        };

        let init_commands = self.state.make_render_commands();
        self.sink()
            .begin_render_pass(init_commands, descriptor);
    }

    fn next_subpass(&mut self, _contents: com::SubpassContents) {
        unimplemented!()
    }

    fn end_render_pass(&mut self) {
        self.sink().stop_encoding();
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        let pipeline_state = pipeline.raw.to_owned();
        self.state.render_pso = Some(pipeline_state.clone());
        self.state.depth_stencil_state = pipeline.depth_stencil_state.as_ref().map(ToOwned::to_owned);
        self.state.primitive_type = pipeline.primitive_type;
        self.state.attribute_buffer_index = pipeline.attribute_buffer_index as usize;

        let mut commands = SmallVec::<[_; 4]>::new();
        commands.push(
            soft::RenderCommand::BindPipeline(pipeline_state, pipeline.depth_stencil_state.clone())
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
        self.sink().pre_render_commands(commands);
    }

    fn bind_graphics_descriptor_sets<'a, T>(
        &mut self,
        layout: &native::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<native::DescriptorSet>,
    {
        use spirv_cross::{msl, spirv};

        let mut commands = Vec::new(); //TODO: re-use the storage

        for (set_index, desc_set) in sets.into_iter().enumerate() {
            let location_vs = msl::ResourceBindingLocation {
                stage: spirv::ExecutionModel::Vertex,
                desc_set: (first_set + set_index) as _,
                binding: 0,
            };
            let location_fs = msl::ResourceBindingLocation {
                stage: spirv::ExecutionModel::Fragment,
                desc_set: (first_set + set_index) as _,
                binding: 0,
            };
            match *desc_set.borrow() {
                native::DescriptorSet::Emulated(ref desc_inner) => {
                    use native::DescriptorSetBinding::*;
                    let set = desc_inner.lock().unwrap();
                    for (&binding, values) in set.bindings.iter() {
                        let desc_layout = set.layout.iter().find(|x| x.binding == binding).unwrap();

                        if desc_layout.stage_flags.contains(pso::ShaderStageFlags::VERTEX) {
                            let res = &layout.res_overrides[&msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_vs
                            }];
                            let resources = &mut self.state.resources_vs;
                            match *values {
                                Sampler(ref samplers) => {
                                    let start = res.sampler_id as usize;
                                    resources.add_samplers(start, samplers.as_slice());
                                    commands.extend(samplers.iter().cloned().enumerate().map(|(i, sampler)| {
                                        soft::RenderCommand::BindSampler {
                                            stage: pso::Stage::Vertex,
                                            index: start + i,
                                            sampler,
                                        }
                                    }));
                                }
                                Image(ref images) => {
                                    let start = res.texture_id as usize;
                                    resources.add_textures(start, images.as_slice());
                                    commands.extend(images.iter().enumerate().map(|(i, texture)| {
                                        soft::RenderCommand::BindTexture {
                                            stage: pso::Stage::Vertex,
                                            index: start + i,
                                            texture: texture.as_ref().map(|&(ref texture, _)| texture.clone()),
                                        }
                                    }));
                                }
                                Combined(ref combos) => {
                                    for (i, combo) in combos.iter().cloned().enumerate() {
                                        let id_tx = res.texture_id as usize + i;
                                        let id_sm = res.sampler_id as usize + i;
                                        let (texture, sampler) = match combo {
                                            Some((ref t, _, ref s)) => (Some(t.clone()), Some(s.clone())),
                                            None => (None, None)
                                        };
                                        resources.add_textures(
                                            id_tx,
                                            &[combo.as_ref().map(|&(ref texture, layout, _)| (texture.clone(), layout))],
                                        );
                                        resources.add_samplers(id_sm, &[sampler.clone()]);
                                        commands.push(soft::RenderCommand::BindTexture {
                                            stage: pso::Stage::Vertex,
                                            index: id_tx,
                                            texture,
                                        });
                                        commands.push(soft::RenderCommand::BindSampler {
                                            stage: pso::Stage::Vertex,
                                            index: id_sm,
                                            sampler,
                                        });
                                    }
                                }
                                Buffer(ref buffers) => {
                                    let start = res.buffer_id as usize;
                                    for (i, bref) in buffers.iter().enumerate() {
                                        let (buffer, offset) = match *bref {
                                            Some((ref buffer, offset)) => (Some(buffer.clone()), offset),
                                            None => (None, 0),
                                        };
                                        commands.push(soft::RenderCommand::BindBuffer {
                                            stage: pso::Stage::Vertex,
                                            index: start + i,
                                            buffer,
                                            offset,
                                        });
                                        if let Some((ref buffer, offset)) = *bref {
                                            resources.add_buffer(start + i, buffer.as_ref(), offset as _);
                                        }
                                    }
                                }
                            }
                        }
                        if desc_layout.stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                            let resources = &mut self.state.resources_fs;
                            let res = &layout.res_overrides[&msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_fs
                            }];
                            match *values {
                                Sampler(ref samplers) => {
                                    let start = res.sampler_id as usize;
                                    resources.add_samplers(start, samplers.as_slice());
                                    commands.extend(samplers.iter().cloned().enumerate().map(|(i, sampler)| {
                                        soft::RenderCommand::BindSampler {
                                            stage: pso::Stage::Fragment,
                                            index: start + i,
                                            sampler,
                                        }
                                    }));
                                }
                                Image(ref images) => {
                                    let start = res.texture_id as usize;
                                    resources.add_textures(start, images.as_slice());
                                    commands.extend(images.iter().enumerate().map(|(i, texture)| {
                                        soft::RenderCommand::BindTexture {
                                            stage: pso::Stage::Fragment,
                                            index: start + i,
                                            texture: texture.as_ref().map(|&(ref texture, _)| texture.clone()),
                                        }
                                    }));
                                }
                                Combined(ref combos) => {
                                    for (i, combo) in combos.iter().cloned().enumerate() {
                                        let id_tx = res.texture_id as usize + i;
                                        let id_sm = res.sampler_id as usize + i;
                                        let (texture, sampler) = match combo {
                                            Some((ref t, _, ref s)) => (Some(t.clone()), Some(s.clone())),
                                            None => (None, None)
                                        };
                                        resources.add_textures(
                                            id_tx,
                                            &[combo.as_ref().map(|&(ref texture, layout, _)| (texture.clone(), layout))],
                                        );
                                        resources.add_samplers(id_sm, &[sampler.clone()]);
                                        commands.push(soft::RenderCommand::BindTexture {
                                            stage: pso::Stage::Fragment,
                                            index: id_tx,
                                            texture,
                                        });
                                        commands.push(soft::RenderCommand::BindSampler {
                                            stage: pso::Stage::Fragment,
                                            index: id_sm,
                                            sampler,
                                        });
                                    }
                                }
                                Buffer(ref buffers) => {
                                    let start = res.buffer_id as usize;
                                    for (i, bref) in buffers.iter().enumerate() {
                                        let (buffer, offset) = match *bref {
                                            Some((ref buffer, offset)) => {
                                                resources.add_buffer(start + i, buffer.as_ref(), offset as _);
                                                (Some(buffer.clone()), offset)
                                            },
                                            None => (None, 0),
                                        };
                                        commands.push(soft::RenderCommand::BindBuffer {
                                            stage: pso::Stage::Fragment,
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
                    if stage_flags.contains(pso::ShaderStageFlags::VERTEX) {
                        let slot = layout.res_overrides[&location_vs].buffer_id;
                        self.state.resources_vs.add_buffer(slot as _, buffer, offset as _);
                        commands.push(soft::RenderCommand::BindBuffer {
                            stage: pso::Stage::Vertex,
                            index: slot as _,
                            buffer: Some(buffer.clone()),
                            offset,
                        });
                    }
                    if stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                        let slot = layout.res_overrides[&location_fs].buffer_id;
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

        self.sink().pre_render_commands(commands);
    }

    fn bind_compute_pipeline(&mut self, pipeline: &native::ComputePipeline) {
        self.state.compute_pso = Some(pipeline.raw.clone());
        self.state.work_group_size = pipeline.work_group_size;

        let command = soft::ComputeCommand::BindPipeline(pipeline.raw.clone());
        self.sink().pre_compute_commands(iter::once(command));
    }

    fn bind_compute_descriptor_sets<'a, T>(
        &mut self,
        layout: &native::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<native::DescriptorSet>,
    {
        use spirv_cross::{msl, spirv};
        let mut commands = Vec::new();

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
                    for (&binding, values) in set.bindings.iter() {
                        let desc_layout = set.layout.iter().find(|x| x.binding == binding).unwrap();

                        if desc_layout.stage_flags.contains(pso::ShaderStageFlags::COMPUTE) {
                            let res = &layout.res_overrides[&msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_cs
                            }];
                            match *values {
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
                                    for (i, combo) in combos.iter().cloned().enumerate() {
                                        let id_tx = res.texture_id as usize + i;
                                        let id_sm = res.sampler_id as usize + i;
                                        let (texture, sampler) = match combo {
                                            Some((ref t, _, ref s)) => (Some(t.clone()), Some(s.clone())),
                                            None => (None, None)
                                        };
                                        resources.add_textures(
                                            id_tx,
                                            &[combo.as_ref().map(|&(ref texture, layout, _)| (texture.clone(), layout))],
                                        );
                                        resources.add_samplers(id_sm, &[sampler.clone()]);
                                        commands.push(soft::ComputeCommand::BindTexture {
                                            index: id_tx,
                                            texture,
                                        });
                                        commands.push(soft::ComputeCommand::BindSampler {
                                            index: id_sm,
                                            sampler,
                                        });
                                    }
                                }
                                Buffer(ref buffers) => {
                                    let start = res.buffer_id as usize;
                                    for (i, bref) in buffers.iter().enumerate() {
                                        let (buffer, offset) = match *bref {
                                            Some((ref buffer, offset)) => {
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

        self.sink().pre_compute_commands(commands);
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

        let sink = self.sink();
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

        let sink = self.sink();
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
        let inner = unsafe { &mut *self.inner.get() };
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

        let mut blit_commands = Vec::new();
        let mut compute_commands = vec![
            soft::ComputeCommand::BindPipeline(compute_pipe),
        ];

        for region in  regions {
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
                let copy_data = self.shared.device.new_buffer_with_data(
                    &(r.size as u32) as *const u32 as *const _,
                    mem::size_of::<u32>() as _,
                    metal::MTLResourceOptions::StorageModeShared,
                );

                inner.retained_buffers.push(copy_data.clone());

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
                compute_commands.push(soft::ComputeCommand::BindBuffer {
                    index: 2,
                    buffer: Some(copy_data),
                    offset: 0,
                });
                compute_commands.push(soft::ComputeCommand::Dispatch {
                    wg_size,
                    wg_count,
                });
            }
        }

        let sink = inner.sink.as_mut().unwrap();
        sink.blit_commands(blit_commands.into_iter());

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
        let commands = regions.into_iter().map(|region| {
            soft::BlitCommand::CopyImage {
                src: src.raw.clone(),
                dst: dst.raw.clone(),
                region: region.borrow().clone(),
            }
        });
        self.sink().blit_commands(commands);
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
                dst: dst.raw.clone(),
                dst_desc: dst.format_desc,
                region: region.borrow().clone(),
            }
        });
        self.sink().blit_commands(commands);
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
                src: src.raw.clone(),
                src_desc: src.format_desc,
                dst: dst.raw.clone(),
                region: region.borrow().clone(),
            }
        });
        self.sink().blit_commands(commands);
    }

    fn draw(
        &mut self,
        vertices: Range<VertexCount>,
        instances: Range<InstanceCount>,
    ) {
        let command = soft::RenderCommand::Draw {
            primitive_type: self.state.primitive_type,
            vertices,
            instances,
        };
        self.sink().render_commands(iter::once(command));
    }

    fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    ) {
        let command = soft::RenderCommand::DrawIndexed {
            index: self.state.index_buffer.clone().expect("must bind index buffer"),
            primitive_type: self.state.primitive_type,
            indices,
            base_vertex,
            instances,
        };
        self.sink().render_commands(iter::once(command));
    }

    fn draw_indirect(
        &mut self,
        _buffer: &native::Buffer,
        _offset: buffer::Offset,
        _draw_count: u32,
        _stride: u32,
    ) {
        unimplemented!()
    }

    fn draw_indexed_indirect(
        &mut self,
        _buffer: &native::Buffer,
        _offset: buffer::Offset,
        _draw_count: u32,
        _stride: u32,
    ) {
        unimplemented!()
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
        _stages: pso::ShaderStageFlags,
        _offset: u32,
        _constants: &[u32],
    ) {
        unimplemented!()
    }

    fn push_compute_constants(
        &mut self,
        _layout: &native::PipelineLayout,
        _offset: u32,
        _constants: &[u32],
    ) {
        unimplemented!()
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
