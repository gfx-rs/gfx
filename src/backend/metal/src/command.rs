use {Backend};
use {native, window};

use std::borrow::{Borrow, BorrowMut};
use std::cell::UnsafeCell;
use std::ops::{Deref, Range};
use std::sync::{Arc};
use std::{iter, mem};

use hal::{buffer, command as com, error, memory, pool, pso};
use hal::{VertexCount, VertexOffset, InstanceCount, IndexCount, WorkGroupCount};
use hal::format::FormatDesc;
use hal::image::{ImageLayout, SubresourceRange};
use hal::query::{Query, QueryControl, QueryId};
use hal::queue::{RawCommandQueue, RawSubmission};

use metal::{self, MTLViewport, MTLScissorRect, MTLPrimitiveType, MTLClearColor, MTLIndexType, MTLSize, MTLOrigin};
use cocoa::foundation::NSUInteger;
use block::{ConcreteBlock};
use conversions::map_index_type;
use soft;


pub struct CommandQueue(pub(crate) Arc<QueueInner>);

pub(crate) struct QueueInner {
    queue: metal::CommandQueue,
}

unsafe impl Send for QueueInner {}
unsafe impl Sync for QueueInner {}

pub struct CommandPool {
    pub(crate) queue: Arc<QueueInner>,
    pub(crate) managed: Option<Vec<CommandBuffer>>,
}

unsafe impl Send for CommandPool {
}
unsafe impl Sync for CommandPool {
}

#[derive(Clone)]
pub struct CommandBuffer {
    inner: Arc<UnsafeCell<CommandBufferInner>>,
    queue: Option<Arc<QueueInner>>,
}

#[derive(Debug)]
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

    fn add_textures(&mut self, start: usize, textures: &[Option<(metal::Texture, ImageLayout)>]) {
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
        I: Iterator<Item = soft::RenderCommand>,
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
            CommandSink::Immediate { ref cmd_buffer, ref mut encoder_state } => {
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
        I: Iterator<Item = soft::ComputeCommand>,
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
}

#[derive(Clone)]
pub struct IndexBuffer {
    buffer: metal::Buffer,
    offset: buffer::Offset,
    index_type: MTLIndexType,
}

struct CommandBufferInner {
    sink: CommandSink,
    // hopefully, this is temporary
    // currently needed for `update_buffer` only
    device: metal::Device,
    //TODO: would be cleaner to move the cache into `CommandBuffer` iself
    // it doesn't have to be in `Inner`
    viewport: Option<MTLViewport>,
    scissors: Option<MTLScissorRect>,
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

impl CommandBufferInner {
    fn reset_resources(&mut self) {
        self.resources_vs.clear();
        self.resources_fs.clear();
        self.resources_cs.clear();
    }

    fn reset(&mut self, queue: &QueueInner, release_resources: bool) {
        match self.sink {
            CommandSink::Immediate { ref mut cmd_buffer, ref mut encoder_state } => {
                //TODO: release the old one?
                *cmd_buffer = queue.queue.new_command_buffer().to_owned();
                *encoder_state = EncoderState::None;
            }
            CommandSink::Deferred { ref mut passes, .. } => {
                passes.clear();
                if release_resources {
                    passes.shrink_to_fit();
                }
            }
        };
        self.reset_resources();
    }

    fn stop_encoding(&mut self) {
        match self.sink {
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

    fn begin_render_pass(&mut self, descriptor: metal::RenderPassDescriptor) {
        self.stop_encoding();

        // TODO: re-use storage
        let mut commands = Vec::new();
        // Apply previously bound values for this command buffer
        commands.extend(self.viewport.map(soft::RenderCommand::SetViewport));
        commands.extend(self.scissors.map(soft::RenderCommand::SetScissor));
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

        match self.sink {
            CommandSink::Immediate { ref cmd_buffer, ref mut encoder_state } => {
                let encoder = cmd_buffer.new_render_command_encoder(&descriptor);
                for command in commands {
                    exec_render(encoder, &command);
                }
                *encoder_state = EncoderState::Render(encoder.to_owned());
            }
            CommandSink::Deferred { ref mut passes, ref mut is_encoding } => {
                *is_encoding = true;
                passes.push(soft::Pass::Render(descriptor, commands));
            }
        }
    }

    /// Start a compute encoder and flush the current state into it,
    /// since Metal doesn't inherit state between passes, and it needs
    /// dispatches to be contained within compute passes.
    ///
    /// Return the current work group size.
    fn begin_compute(&mut self) -> MTLSize {
        self.stop_encoding(); //TODO: don't do this
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

        match self.sink {
            CommandSink::Immediate { ref cmd_buffer, ref mut encoder_state } => {
                let encoder = cmd_buffer.new_compute_command_encoder();
                for command in commands {
                    exec_compute(encoder, &command);
                }
                *encoder_state = EncoderState::Compute(encoder.to_owned());
            }
            CommandSink::Deferred { ref mut passes, ref mut is_encoding } => {
                *is_encoding = true;
                passes.push(soft::Pass::Compute(commands));
            }
        }

        self.work_group_size
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

fn compute_pitches(region: &com::BufferImageCopy, fd: &FormatDesc) -> (u32, u32) {
    let row_pitch = div(region.buffer_width, fd.dim.0 as _) * (fd.bits / 8) as u32;
    let slice_pitch = div(region.buffer_height, fd.dim.1 as _) * row_pitch;
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
        Cmd::Draw { primitive_type, ref vertices, ref instances } => unsafe {
            msg_send![*encoder,
                drawPrimitives: primitive_type
                vertexStart: vertices.start as NSUInteger
                vertexCount: (vertices.end - vertices.start) as NSUInteger
                instanceCount: (instances.end - instances.start) as NSUInteger
                baseInstance: instances.start as NSUInteger
            ];
        }
        Cmd::DrawIndexed { ref index, primitive_type, ref indices, base_vertex, ref instances } => {
            let index_offset = indices.start as buffer::Offset * match index.index_type {
                MTLIndexType::UInt16 => 2,
                MTLIndexType::UInt32 => 4,
            };
            // Metal requires `indexBufferOffset` alignment of 4
            assert_eq!((index_offset + index.offset) & 3, 0);
            unsafe {
                msg_send![*encoder,
                    drawIndexedPrimitives: primitive_type
                    indexCount: (indices.end - indices.start) as NSUInteger
                    indexType: index.index_type
                    indexBuffer: index.buffer.as_ref()
                    indexBufferOffset: (index_offset + index.offset) as NSUInteger
                    instanceCount: (instances.end - instances.start) as NSUInteger
                    baseVertex: base_vertex as NSUInteger
                    baseInstance: instances.start as NSUInteger
                ];
            }
        }
    }
}

fn exec_blit(encoder: &metal::BlitCommandEncoderRef, command: &soft::BlitCommand) {
    use soft::BlitCommand as Cmd;
    match *command {
        Cmd::CopyBuffer { ref src, ref dst, ref region } => unsafe {
            msg_send![*encoder,
                copyFromBuffer: src.as_ref()
                sourceOffset: region.src as NSUInteger
                toBuffer: dst.as_ref()
                destinationOffset: region.dst as NSUInteger
                size: region.size as NSUInteger
            ];
        },
        Cmd::CopyBufferToImage { ref src, ref dst, dst_desc, ref region } => unsafe {
            let (row_pitch, slice_pitch) = compute_pitches(&region, &dst_desc);
            let image_offset = &region.image_offset;
            let r = &region.image_layers;
            let extent = MTLSize {
                width: dst.width(),
                height: dst.height(),
                depth: dst.depth(),
            };

            for layer in r.layers.clone() {
                let offset = region.buffer_offset + slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                msg_send![*encoder,
                    copyFromBuffer: src.as_ref()
                    sourceOffset: offset as NSUInteger
                    sourceBytesPerRow: row_pitch as NSUInteger
                    sourceBytesPerImage: slice_pitch as NSUInteger
                    sourceSize: extent
                    toTexture: dst.as_ref()
                    destinationSlice: layer as NSUInteger
                    destinationLevel: r.level as NSUInteger
                    destinationOrigin: MTLOrigin { x: image_offset.x as _, y: image_offset.y as _, z: image_offset.z as _ }
                ]
            }
        },
        Cmd::CopyImageToBuffer { ref src, src_desc, ref dst, ref region } => unsafe {
            let (row_pitch, slice_pitch) = compute_pitches(&region, &src_desc);
            let image_offset = &region.image_offset;
            let r = &region.image_layers;
            let extent = MTLSize {
                width: src.width(),
                height: src.height(),
                depth: src.depth(),
            };

            for layer in r.layers.clone() {
                let offset = region.buffer_offset + slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                msg_send![*encoder,
                    copyFromTexture: src.as_ref()
                    sourceSlice: layer as NSUInteger
                    sourceLevel: r.level as NSUInteger
                    sourceOrigin: MTLOrigin { x: image_offset.x as _, y: image_offset.y as _, z: image_offset.z as _ }
                    sourceSize: extent
                    toBuffer: dst.as_ref()
                    destinationOffset: offset as NSUInteger
                    destinationBytesPerRow: row_pitch as NSUInteger
                    destinationBytesPerImage: slice_pitch as NSUInteger
                ]
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

impl CommandQueue {
    pub fn new(device: &metal::DeviceRef) -> CommandQueue {
        CommandQueue(Arc::new(QueueInner {
            queue: device.new_command_queue(),
        }))
    }

    pub unsafe fn device(&self) -> &metal::DeviceRef {
        msg_send![&*self.0.queue, device]
    }
}

impl RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw<IC>(&mut self, submit: RawSubmission<Backend, IC>, fence: Option<&native::Fence>)
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
            Some(ConcreteBlock::new(move |_cb: *mut ()| -> () {
                for semaphore in semaphores_copy.iter() {
                    native::dispatch_semaphore_signal(*semaphore);
                }
            }).copy())
        } else {
            None
        };

        let buffers = submit.cmd_buffers.into_iter().collect::<Vec<_>>();
        let num_buffers = buffers.len();
        for (i, buffer) in buffers.into_iter().enumerate() {
            let buffer = buffer.borrow();
            let command_buffer: &metal::CommandBufferRef = match buffer.inner_ref().sink {
                 CommandSink::Immediate { ref cmd_buffer, .. } => cmd_buffer,
                 CommandSink::Deferred { ref passes, .. } => {
                    let cmd_buffer = self.0.queue.new_command_buffer();
                    record_commands(cmd_buffer, passes);
                    cmd_buffer
                 }
            };
            if let Some(ref signal_block) = signal_block {
                msg_send![command_buffer, addCompletedHandler: signal_block.deref() as *const _];
            }
            // only append the fence handler to the last buffer
            if i + 1 == num_buffers {
                if let Some(ref fence) = fence {
                    let value_ptr = fence.0.clone();
                    let fence_block = ConcreteBlock::new(move |_cb: *mut ()| -> () {
                        *value_ptr.lock().unwrap() = true;
                    }).copy();
                    msg_send![command_buffer, addCompletedHandler: fence_block.deref() as *const _];
                }
            }
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
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        unimplemented!()
    }
}

impl pool::RawCommandPool<Backend> for CommandPool {
    fn reset(&mut self) {
        if let Some(ref mut managed) = self.managed {
            for cmd_buffer in managed {
                cmd_buffer.inner().reset(&self.queue, false);
            }
        }
    }

    fn allocate(&mut self, num: usize, _level: com::RawLevel) -> Vec<CommandBuffer> { //TODO: Implement secondary buffers
        let buffers: Vec<_> = (0..num).map(|_| CommandBuffer {
            inner: Arc::new({
                UnsafeCell::new(CommandBufferInner {
                    sink: CommandSink::Immediate {
                        cmd_buffer: self.queue.queue.new_command_buffer().to_owned(),
                        encoder_state: EncoderState::None,
                    },
                    device: unsafe {
                        CommandQueue(self.queue.clone()).device().to_owned()
                    },
                    viewport: None,
                    scissors: None,
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
                })
            }),
            queue: if self.managed.is_some() {
                None
            } else {
                Some(self.queue.clone())
            },
        }).collect();

        if let Some(ref mut managed) = self.managed {
            managed.extend_from_slice(&buffers);
        }
        buffers
    }

    /// Free command buffers which are allocated from this pool.
    unsafe fn free(&mut self, buffers: Vec<CommandBuffer>) {
        let managed = match self.managed {
            Some(ref mut vec) => vec,
            None => return,
        };
        for cmd_buf in buffers {
            //TODO: what else here?
            let inner_ptr = cmd_buf.inner.get();
            match managed.iter_mut().position(|b| inner_ptr == b.inner.get()) {
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
    #[inline]
    fn inner(&mut self) -> &mut CommandBufferInner {
        unsafe {
            &mut *self.inner.get()
        }
    }

    #[inline]
    fn inner_ref(&self) -> &CommandBufferInner {
        unsafe {
            &*self.inner.get()
        }
    }

    #[inline]
    pub fn device(&self) -> &metal::DeviceRef {
        &self.inner_ref().device
    }
}

impl com::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self, flags: com::CommandBufferFlags) {
        let inner = unsafe { &mut *self.inner.get() };
        inner.reset_resources();

        if flags.contains(com::CommandBufferFlags::ONE_TIME_SUBMIT) {
            if let Some(ref queue) = self.queue {
                inner.sink = CommandSink::Immediate {
                    cmd_buffer: queue.queue.new_command_buffer().to_owned(),
                    encoder_state: EncoderState::None,
                };
            }
            //TODO: assert(CommandSink::Immediate);
        } else {
            let passes_storage = match inner.sink {
                CommandSink::Immediate { .. } => {
                    //TODO: release resources?
                    Some(Vec::new())
                }
                CommandSink::Deferred { ref mut passes, .. } => {
                    passes.clear();
                    None
                }
            };
            if let Some(passes) = passes_storage {
                inner.sink = CommandSink::Deferred { passes, is_encoding: false };
            }
        }
    }

    fn finish(&mut self) {
        self.inner().stop_encoding();
    }

    fn reset(&mut self, release_resources: bool) {
        let queue = self.queue.as_ref()
            .expect("unable to reset an individual command buffer from a pool that doesn't support that");
        unsafe { &mut *self.inner.get() }
            .reset(queue, release_resources);
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

    fn fill_buffer(
        &mut self,
        _buffer: &native::Buffer,
        _range: Range<buffer::Offset>,
        _data: u32,
    ) {
        unimplemented!()
    }

    fn update_buffer(
        &mut self,
        dst: &native::Buffer,
        offset: buffer::Offset,
        data: &[u8],
    ) {
        let inner = self.inner();
        //TODO: allocate from command pool, don't retain automatically
        let src = inner.device.new_buffer_with_data(
            data.as_ptr() as _,
            data.len() as _,
            metal::MTLResourceOptions::StorageModePrivate,
        );
        let command = soft::BlitCommand::CopyBuffer {
            src,
            dst: dst.raw.clone(),
            region: com::BufferCopy {
                src: 0,
                dst: offset,
                size: data.len() as _,
            },
        };
        inner.sink.blit_commands(iter::once(command));
    }

    fn clear_color_image_raw(
        &mut self,
        _image: &native::Image,
        _layout: ImageLayout,
        _range: SubresourceRange,
        _value: com::ClearColorRaw,
    ) {
        unimplemented!()
    }

    fn clear_depth_stencil_image_raw(
        &mut self,
        _image: &native::Image,
        _layout: ImageLayout,
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
        U::Item: Borrow<com::Rect>,
    {
        unimplemented!()
    }

    fn resolve_image<T>(
        &mut self,
        _src: &native::Image,
        _src_layout: ImageLayout,
        _dst: &native::Image,
        _dst_layout: ImageLayout,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageResolve>,
    {
        unimplemented!()
    }

    fn blit_image<T>(
        &mut self,
        _src: &native::Image,
        _src_layout: ImageLayout,
        _dst: &native::Image,
        _dst_layout: ImageLayout,
        _filter: com::BlitFilter,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageBlit>
    {
        unimplemented!()
    }

    fn bind_index_buffer(&mut self, view: buffer::IndexBufferView<Backend>) {
        let buffer = view.buffer.raw.clone();
        let offset = view.offset;
        let index_type = map_index_type(view.index_type);
        self.inner().index_buffer = Some(IndexBuffer {
            buffer,
            offset,
            index_type,
        });
    }

    fn bind_vertex_buffers(&mut self, buffer_set: pso::VertexBufferSet<Backend>) {
        let inner = self.inner();
        let buffers = &mut inner.resources_vs.buffers;
        while buffers.len() < inner.attribute_buffer_index + buffer_set.0.len()    {
            buffers.push(None)
        }
        for (ref mut out, &(ref buffer, offset)) in buffers[inner.attribute_buffer_index..].iter_mut().zip(buffer_set.0.iter()) {
            **out = Some((buffer.raw.clone(), offset));
        }

        let attribute_buffer_index = inner.attribute_buffer_index;
        let commands = buffer_set.0.iter().enumerate().map(|(i, &(buffer, offset))| {
            soft::RenderCommand::BindBuffer {
                stage: pso::Stage::Vertex,
                index: attribute_buffer_index + i,
                buffer: Some(buffer.raw.clone()),
                offset,
            }
        });
        inner.sink.pre_render_commands(commands);
    }

    fn set_viewports<T>(&mut self, vps: T)
    where
        T: IntoIterator,
        T::Item: Borrow<com::Viewport>,
    {
        let mut vps = vps.into_iter();
        let vp_borrowable = vps.next().expect("No viewport provided, Metal supports exactly one");
        let vp = vp_borrowable.borrow();
        if vps.next().is_some() {
            panic!("Metal supports only one viewport");
        }
        let inner = self.inner();
        let viewport = MTLViewport {
            originX: vp.rect.x as f64,
            originY: vp.rect.y as f64,
            width: vp.rect.w as f64,
            height: vp.rect.h as f64,
            znear: vp.depth.start as f64,
            zfar: vp.depth.end as f64,
        };
        inner.viewport = Some(viewport);
        let command = soft::RenderCommand::SetViewport(viewport);
        inner.sink.pre_render_commands(iter::once(command));
    }

    fn set_scissors<T>(&mut self, rects: T)
    where
        T: IntoIterator,
        T::Item: Borrow<com::Rect>,
    {
        let mut rects = rects.into_iter();
        let rect_borrowable = rects.next().expect("No scissor provided, Metal supports exactly one");
        let rect = rect_borrowable.borrow();
        if rects.next().is_some() {
            panic!("Metal supports only one scissor");
        }
        let inner = self.inner();
        let scissor = MTLScissorRect {
            x: rect.x as _,
            y: rect.y as _,
            width: rect.w as _,
            height: rect.h as _,
        };
        inner.scissors = Some(scissor);
        let command = soft::RenderCommand::SetScissor(scissor);
        inner.sink.pre_render_commands(iter::once(command));
    }

    fn set_stencil_reference(&mut self, _front: com::StencilValue, _back: com::StencilValue) {
        unimplemented!()
    }

    fn set_blend_constants(&mut self, _color: com::ColorValue) {
        unimplemented!()
    }

    fn begin_render_pass_raw<T>(
        &mut self,
        render_pass: &native::RenderPass,
        frame_buffer: &native::FrameBuffer,
        _render_area: com::Rect,
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

        self.inner().begin_render_pass(descriptor);
    }

    fn next_subpass(&mut self, _contents: com::SubpassContents) {
        unimplemented!()
    }

    fn end_render_pass(&mut self) {
        self.inner().stop_encoding();
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        let inner = self.inner();
        let pipeline_state = pipeline.raw.to_owned();
        let command = soft::RenderCommand::BindPipeline(pipeline_state.clone(), pipeline.depth_stencil_state.clone());
        inner.sink.pre_render_commands(iter::once(command));

        inner.render_pso = Some(pipeline_state);
        inner.depth_stencil_state = pipeline.depth_stencil_state.as_ref().map(ToOwned::to_owned);
        inner.primitive_type = pipeline.primitive_type;
        inner.attribute_buffer_index = pipeline.attribute_buffer_index as usize;
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

        let inner = self.inner();
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
                            let location = msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_vs
                            };
                            let start = layout.res_overrides[&location].resource_id as usize;
                            match *values {
                                Sampler(ref samplers) => {
                                    inner.resources_vs.add_samplers(start, samplers.as_slice());
                                    commands.extend(samplers.iter().cloned().enumerate().map(|(i, sampler)| {
                                        soft::RenderCommand::BindSampler {
                                            stage: pso::Stage::Vertex,
                                            index: start + i,
                                            sampler,
                                        }
                                    }));
                                }
                                Image(ref images) => {
                                    inner.resources_vs.add_textures(start, images.as_slice());
                                    commands.extend(images.iter().enumerate().map(|(i, texture)| {
                                        soft::RenderCommand::BindTexture {
                                            stage: pso::Stage::Vertex,
                                            index: start + i,
                                            texture: texture.as_ref().map(|&(ref texture, _)| texture.clone()),
                                        }
                                    }));
                                }
                                Buffer(ref buffers) => {
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
                                            inner.resources_vs.add_buffer(start + i, buffer.as_ref(), offset as _);
                                        }
                                    }
                                }
                            }
                        }
                        if desc_layout.stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                            let location = msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_fs
                            };
                            let start = layout.res_overrides[&location].resource_id as usize;
                            match *values {
                                Sampler(ref samplers) => {
                                    inner.resources_fs.add_samplers(start, samplers.as_slice());
                                    commands.extend(samplers.iter().cloned().enumerate().map(|(i, sampler)| {
                                        soft::RenderCommand::BindSampler {
                                            stage: pso::Stage::Fragment,
                                            index: start + i,
                                            sampler,
                                        }
                                    }));
                                }
                                Image(ref images) => {
                                    inner.resources_fs.add_textures(start, images.as_slice());
                                    commands.extend(images.iter().enumerate().map(|(i, texture)| {
                                        soft::RenderCommand::BindTexture {
                                            stage: pso::Stage::Fragment,
                                            index: start + i,
                                            texture: texture.as_ref().map(|&(ref texture, _)| texture.clone()),
                                        }
                                    }));
                                }
                                Buffer(ref buffers) => {
                                    for (i, bref) in buffers.iter().enumerate() {
                                        let (buffer, offset) = match *bref {
                                            Some((ref buffer, offset)) => {
                                                inner.resources_fs.add_buffer(start + i, buffer.as_ref(), offset as _);
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
                        let slot = layout.res_overrides[&location_vs].resource_id;
                        inner.resources_vs.add_buffer(slot as _, buffer, offset as _);
                        commands.push(soft::RenderCommand::BindBuffer {
                            stage: pso::Stage::Vertex,
                            index: slot as _,
                            buffer: Some(buffer.clone()),
                            offset,
                        });
                    }
                    if stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                        let slot = layout.res_overrides[&location_fs].resource_id;
                        inner.resources_fs.add_buffer(slot as _, &buffer, offset as _);
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

        inner.sink.pre_render_commands(commands.into_iter());
    }

    fn bind_compute_pipeline(&mut self, pipeline: &native::ComputePipeline) {
        let inner = self.inner();
        inner.compute_pso = Some(pipeline.raw.clone());
        inner.work_group_size = pipeline.work_group_size;

        let command = soft::ComputeCommand::BindPipeline(pipeline.raw.clone());
        inner.sink.pre_compute_commands(iter::once(command));
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

        let inner = self.inner();
        let resources = &mut inner.resources_cs;
        let mut commands = Vec::new();

        for (set_index, desc_set) in sets.into_iter().enumerate() {
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
                            let location = msl::ResourceBindingLocation {
                                binding: binding as _,
                                .. location_cs
                            };
                            let start = layout.res_overrides[&location].resource_id as usize;
                            match *values {
                                Sampler(ref samplers) => {
                                    resources.add_samplers(start, samplers.as_slice());
                                    commands.extend(samplers.iter().cloned().enumerate().map(|(i, sampler)| {
                                        soft::ComputeCommand::BindSampler {
                                            index: start + i,
                                            sampler,
                                        }
                                    }));
                                }
                                Image(ref images) => {
                                    resources.add_textures(start, images.as_slice());
                                    commands.extend(images.iter().enumerate().map(|(i, texture)| {
                                        soft::ComputeCommand::BindTexture {
                                            index: start + i,
                                            texture: texture.as_ref().map(|&(ref texture, _)| texture.clone()),
                                        }
                                    }));
                                }
                                Buffer(ref buffers) => {
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
                        let slot = layout.res_overrides[&location_cs].resource_id;
                        resources.add_buffer(slot as _, buffer, offset as _);
                    }
                }
            }
        }

        inner.sink.pre_compute_commands(commands.into_iter());
    }

    fn dispatch(&mut self, count: WorkGroupCount) {
        let inner = self.inner();

        let command = soft::ComputeCommand::Dispatch {
            wg_size: inner.begin_compute(),
            wg_count: MTLSize {
                width: count[0] as _,
                height: count[1] as _,
                depth: count[2] as _,
            },
        };
        inner.sink.compute_commands(iter::once(command));

        //TODO: re-use compute encoders
        inner.stop_encoding();
    }

    fn dispatch_indirect(&mut self, buffer: &native::Buffer, offset: buffer::Offset) {
        let inner = self.inner();

        let command = soft::ComputeCommand::DispatchIndirect {
            wg_size: inner.begin_compute(),
            buffer: buffer.raw.clone(),
            offset,
        };
        inner.sink.compute_commands(iter::once(command));

        //TODO: re-use compute encoders
        inner.stop_encoding();
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
        let commands = regions.into_iter().map(|region| {
            soft::BlitCommand::CopyBuffer {
                src: src.raw.clone(),
                dst: dst.raw.clone(),
                region: region.borrow().clone(),
            }
        });
        self.inner().sink.blit_commands(commands);
    }

    fn copy_image<T>(
        &mut self,
        _src: &native::Image,
        _src_layout: ImageLayout,
        _dst: &native::Image,
        _dst_layout: ImageLayout,
        _regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<com::ImageCopy>,
    {
        unimplemented!()
    }

    fn copy_buffer_to_image<T>(
        &mut self,
        src: &native::Buffer,
        dst: &native::Image,
        _dst_layout: ImageLayout,
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
                dst_desc: dst.format_desc.clone(),
                region: region.borrow().clone(),
            }
        });
        self.inner().sink.blit_commands(commands);
    }

    fn copy_image_to_buffer<T>(
        &mut self,
        src: &native::Image,
        _src_layout: ImageLayout,
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
                src_desc: src.format_desc.clone(),
                dst: dst.raw.clone(),
                region: region.borrow().clone(),
            }
        });
        self.inner().sink.blit_commands(commands);
    }

    fn draw(
        &mut self,
        vertices: Range<VertexCount>,
        instances: Range<InstanceCount>,
    ) {
        let inner = self.inner();
        let command = soft::RenderCommand::Draw {
            primitive_type: inner.primitive_type,
            vertices,
            instances,
        };
        inner.sink.render_commands(iter::once(command));
    }

    fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    ) {
        let inner = self.inner();
        let command = soft::RenderCommand::DrawIndexed {
            index: inner.index_buffer.clone().expect("must bind index buffer"),
            primitive_type: inner.primitive_type,
            indices,
            base_vertex,
            instances,
        };
        inner.sink.render_commands(iter::once(command));
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
