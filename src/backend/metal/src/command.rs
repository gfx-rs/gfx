use {Backend};
use {native, window};

use std::borrow::{Borrow, BorrowMut};
use std::cell::UnsafeCell;
use std::ops::{Deref, Range};
use std::sync::{Arc};
use std::mem;

use hal::{error, memory, pool, pso};
use hal::{VertexCount, VertexOffset, InstanceCount, IndexCount};
use hal::buffer::{IndexBufferView};
use hal::command as com;
use hal::image::{ImageLayout, SubresourceRange};
use hal::query::{Query, QueryControl, QueryId};
use hal::queue::{RawCommandQueue, RawSubmission};

use metal::{self, MTLViewport, MTLScissorRect, MTLPrimitiveType, MTLClearColor, MTLIndexType, MTLSize, MTLOrigin};
use cocoa::foundation::NSUInteger;
use block::{ConcreteBlock};
use conversions::map_index_type;


fn div(a: u32, b: u32) -> u32 {
    assert_eq!(a % b, 0);
    a / b
}

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
    buffers: Vec<Option<(metal::Buffer, pso::BufferOffset)>>,
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

    fn add_buffer(&mut self, slot: usize, buffer: &metal::BufferRef, offset: usize) {
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

struct CommandBufferInner {
    command_buffer: metal::CommandBuffer,
    //TODO: would be cleaner to move the cache into `CommandBuffer` iself
    // it doesn't have to be in `Inner`
    encoder_state: EncoderState,
    viewport: Option<MTLViewport>,
    scissors: Option<MTLScissorRect>,
    render_pso: Option<metal::RenderPipelineState>,
    compute_pso: Option<metal::ComputePipelineState>,
    work_group_size: MTLSize,
    primitive_type: MTLPrimitiveType,
    resources_vs: StageResources,
    resources_fs: StageResources,
    resources_cs: StageResources,
    index_buffer: Option<(metal::Buffer, u64, MTLIndexType)>,
    attribute_buffer_index: usize,
    depth_stencil_state: Option<metal::DepthStencilState>,
}

impl CommandBufferInner {
    fn reset(&mut self, queue: &QueueInner) {
        self.command_buffer = queue.queue.new_command_buffer().to_owned();

        self.resources_vs.clear();
        self.resources_fs.clear();
        self.resources_cs.clear();
    }

    fn stop_ecoding(&mut self) {
        match mem::replace(&mut self.encoder_state, EncoderState::None)  {
            EncoderState::None => {}
            EncoderState::Blit(ref blit_encoder) => {
                blit_encoder.end_encoding();
            }
            EncoderState::Render(ref render_encoder) => {
                render_encoder.end_encoding();
            }
        }
    }

    fn begin_render_pass(&mut self, encoder: metal::RenderCommandEncoder) {
        self.stop_ecoding();

        self.encoder_state = EncoderState::Render(encoder);
        let encoder = if let EncoderState::Render(ref encoder) = self.encoder_state {
            encoder
        } else {
            unreachable!()
        };

        // Apply previously bound values for this command buffer
        if let Some(viewport) = self.viewport {
            encoder.set_viewport(viewport);
        }
        if let Some(scissors) = self.scissors {
            encoder.set_scissor_rect(scissors);
        }
        if let Some(ref pipeline_state) = self.render_pso {
            encoder.set_render_pipeline_state(pipeline_state);
        }
        if let Some(ref depth_stencil_state) = self.depth_stencil_state {
            encoder.set_depth_stencil_state(depth_stencil_state);
        }
        // inherit vertex resources
        for (i, resource) in self.resources_vs.buffers.iter().enumerate() {
            if let Some((ref buffer, offset)) = *resource {
                encoder.set_vertex_buffer(i as _, offset as _, Some(buffer));
            }
        }
        for (i, resource) in self.resources_vs.textures.iter().enumerate() {
            if let Some(ref texture) = *resource {
                encoder.set_vertex_texture(i as _, Some(texture));
            }
        }
        for (i, resource) in self.resources_vs.samplers.iter().enumerate() {
            if let Some(ref sampler) = *resource {
                encoder.set_vertex_sampler_state(i as _, Some(sampler));
            }
        }
        // inherit fragment resources
        for (i, resource) in self.resources_fs.buffers.iter().enumerate() {
            if let Some((ref buffer, offset)) = *resource {
                encoder.set_fragment_buffer(i as _, offset as _, Some(buffer));
            }
        }
        for (i, resource) in self.resources_fs.textures.iter().enumerate() {
            if let Some(ref texture) = *resource {
                encoder.set_fragment_texture(i as _, Some(texture));
            }
        }
        for (i, resource) in self.resources_fs.samplers.iter().enumerate() {
            if let Some(ref sampler) = *resource {
                encoder.set_fragment_sampler_state(i as _, Some(sampler));
            }
        }
    }

    fn begin_compute(&mut self) -> (&metal::ComputeCommandEncoderRef, MTLSize) {
        self.stop_ecoding();

        let encoder = self.command_buffer.new_compute_command_encoder();
        encoder.set_compute_pipeline_state(self.compute_pso.as_ref().unwrap());

        for (i, resource) in self.resources_cs.buffers.iter().enumerate() {
            if let Some((ref buffer, offset)) = *resource {
                encoder.set_buffer(i as _, offset as _, Some(buffer));
            }
        }
        for (i, resource) in self.resources_cs.textures.iter().enumerate() {
            if let Some(ref texture) = *resource {
                encoder.set_texture(i as _, Some(texture));
            }
        }
        for (i, resource) in self.resources_cs.samplers.iter().enumerate() {
            if let Some(ref sampler) = *resource {
                encoder.set_sampler_state(i as _, Some(sampler));
            }
        }

        (encoder, self.work_group_size)
    }
}

unsafe impl Send for CommandBuffer {
}

enum EncoderState {
    None,
    Blit(metal::BlitCommandEncoder),
    Render(metal::RenderCommandEncoder),
    //TODO: Compute() if we find cases where
    // grouping compute-related calls is feasible
}

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
        let mut i = 1;
        for buffer in buffers {
            let buffer = buffer.borrow();
            let command_buffer: &metal::CommandBufferRef = &(&mut *buffer.inner.get()).command_buffer;
            if let Some(ref signal_block) = signal_block {
                msg_send![command_buffer, addCompletedHandler: signal_block.deref() as *const _];
            }
            // only append the fence handler to the last buffer
            if i == num_buffers {
                if let Some(ref fence) = fence {
                    let value_ptr = fence.0.clone();
                    let fence_block = ConcreteBlock::new(move |_cb: *mut ()| -> () {
                        *value_ptr.lock().unwrap() = true;
                    }).copy();
                    msg_send![command_buffer, addCompletedHandler: fence_block.deref() as *const _];
                }
            }
            command_buffer.commit();
            i += 1;
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
                let render_layer_borrow = surface.render_layer.borrow_mut();
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
                cmd_buffer.inner().reset(&self.queue);
            }
        }
    }

    fn allocate(&mut self, num: usize, _level: com::RawLevel) -> Vec<CommandBuffer> { //TODO: Implement secondary buffers
        let buffers: Vec<_> = (0..num).map(|_| CommandBuffer {
            inner: Arc::new({
                // TODO: maybe use unretained command buffer for efficiency?
                let command_buffer = self.queue.queue.new_command_buffer().to_owned();

                UnsafeCell::new(CommandBufferInner {
                    command_buffer,
                    encoder_state: EncoderState::None,
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
        for mut cmd_buf in buffers {
            //TODO: what else here?
            let target = &*cmd_buf.inner().command_buffer;
            let managed = match self.managed {
                Some(ref mut vec) => vec,
                None => continue,
            };
            match managed.iter_mut().position(|b| &*b.inner().command_buffer as *const metal::CommandBufferRef == target as *const metal::CommandBufferRef) {
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

    fn encode_blit(&mut self) -> &metal::BlitCommandEncoderRef {
        let inner = self.inner();
        match inner.encoder_state {
            EncoderState::None => {},
            EncoderState::Blit(ref blit_encoder) => return blit_encoder,
            EncoderState::Render(_) => panic!("invalid inside renderpass"),
        }

        let blit_encoder = inner.command_buffer.new_blit_command_encoder().to_owned();
        inner.encoder_state = EncoderState::Blit(blit_encoder);
        if let EncoderState::Blit(ref blit_encoder) = inner.encoder_state {
            blit_encoder
        } else {
            unreachable!()
        }
    }

    fn expect_render_pass(&self) -> &metal::RenderCommandEncoderRef {
        if let EncoderState::Render(ref encoder) = self.inner_ref().encoder_state {
            encoder
        } else {
            panic!("only valid inside renderpass")
        }
    }

    pub fn device(&self) -> &metal::DeviceRef {
        unsafe {
            msg_send![self.inner_ref().command_buffer, device]
        }
    }
}

impl com::RawCommandBuffer<Backend> for CommandBuffer {
    fn begin(&mut self, _flags: com::CommandBufferFlags) { // TODO: Implement flags somehow
        if let Some(ref queue) = self.queue {
            unsafe { &mut *self.inner.get() }
                .reset(queue);
        }
    }

    fn finish(&mut self) {
        self.inner().stop_ecoding();
    }

    fn reset(&mut self, _release_resources: bool) {
        unsafe { &mut *self.inner.get() }
            .reset(self.queue.as_ref().unwrap());
    }

    fn pipeline_barrier<'a, T>(
        &mut self,
        _stages: Range<pso::PipelineStage>,
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
        _range: Range<u64>,
        _data: u32,
    ) {
        unimplemented!()
    }

    fn update_buffer(
        &mut self,
        dst: &native::Buffer,
        offset: u64,
        data: &[u8],
    ) {
        let src = self.device().new_buffer_with_data(data.as_ptr() as _, data.len() as _, metal::MTLResourceOptions::StorageModePrivate);
        let encoder = self.encode_blit();

        unsafe {
            msg_send![encoder,
                copyFromBuffer: &*src
                sourceOffset: 0 as NSUInteger
                toBuffer: &*dst.raw
                destinationOffset: offset as NSUInteger
                size: data.len() as NSUInteger
            ];
        }
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

    fn bind_index_buffer(&mut self, view: IndexBufferView<Backend>) {
        let buffer = view.buffer.raw.clone();
        let offset = view.offset;
        let index_type = map_index_type(view.index_type);
        self.inner().index_buffer = Some((
            buffer,
            offset,
            index_type,
        ));
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
        //TODO: reuse the binding code from the cache to state between this and `begin_renderpass`
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            for (i, &(buffer, offset)) in buffer_set.0.iter().enumerate() {
                let msl_buffer_index = inner.attribute_buffer_index + i;
                encoder.set_vertex_buffer(msl_buffer_index as _, offset as _, Some(&buffer.raw));
            }
        }
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
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            encoder.set_viewport(viewport);
        }
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
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            encoder.set_scissor_rect(scissor);
        }
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
        let inner = self.inner();

        let encoder = unsafe {
            match inner.encoder_state {
                EncoderState::Render(_) => panic!("already in a renderpass"),
                EncoderState::Blit(ref blit) => {
                    blit.end_encoding();
                },
                EncoderState::None => {},
            }
            inner.encoder_state = EncoderState::None;

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

            inner.command_buffer
                .new_render_command_encoder(&pass_descriptor)
                .to_owned()
        };

        inner.begin_render_pass(encoder);
    }

    fn next_subpass(&mut self, _contents: com::SubpassContents) {
        unimplemented!()
    }

    fn end_render_pass(&mut self) {
        match self.inner().encoder_state {
            EncoderState::Render(ref encoder) => {
                encoder.end_encoding();
            },
            _ => panic!("not in a renderpass"),
        }
        self.inner().encoder_state = EncoderState::None;
    }

    fn bind_graphics_pipeline(&mut self, pipeline: &native::GraphicsPipeline) {
        let inner = self.inner();
        let pipeline_state = pipeline.raw.to_owned();
        if let EncoderState::Render(ref encoder) = inner.encoder_state {
            encoder.set_render_pipeline_state(&pipeline_state);
            if let Some(ref depth_stencil_state) = pipeline.depth_stencil_state {
                encoder.set_depth_stencil_state(depth_stencil_state);
            }
        }
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
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, ref sampler) in samplers.iter().enumerate() {
                                            encoder.set_vertex_sampler_state((start + i) as _, sampler.as_ref().map(|x| &**x));
                                        }
                                    }
                                }
                                Image(ref images) => {
                                    inner.resources_vs.add_textures(start, images.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, ref texture) in images.iter().enumerate() {
                                            encoder.set_vertex_texture((start + i) as _, texture.as_ref().map(|&(ref texture, _)| &**texture));
                                        }
                                    }
                                }
                                Buffer(ref buffers) => {
                                    for (i, ref bref) in buffers.iter().enumerate() {
                                        if let Some((ref buffer, offset)) = **bref {
                                            inner.resources_vs.add_buffer(start + i, buffer.as_ref(), offset as _);
                                            if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                                encoder.set_vertex_buffer((start + i) as _,offset as _, Some(buffer.as_ref()));
                                            }
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
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, sampler) in samplers.iter().enumerate() {
                                            encoder.set_fragment_sampler_state((start + i) as _, sampler.as_ref().map(|x| &**x));
                                        }
                                    }
                                }
                                Image(ref images) => {
                                    inner.resources_fs.add_textures(start, images.as_slice());
                                    if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                        for (i, texture) in images.iter().enumerate() {
                                            encoder.set_fragment_texture((start + i) as _, texture.as_ref().map(|&(ref texture, _)| &**texture));
                                        }
                                    }
                                }
                                Buffer(ref buffers) => {
                                    for (i, ref bref) in buffers.iter().enumerate() {
                                        if let Some((ref buffer, offset)) = **bref {
                                            inner.resources_fs.add_buffer(start + i, buffer.as_ref(), offset as _);
                                            if let EncoderState::Render(ref encoder) = inner.encoder_state {
                                                encoder.set_fragment_buffer((start + i) as _,offset as _, Some(buffer.as_ref()));
                                            }
                                        }
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
                        if let EncoderState::Render(ref encoder) = inner.encoder_state {
                            encoder.set_vertex_buffer(slot as _, offset as _, Some(buffer))
                        }
                    }
                    if stage_flags.contains(pso::ShaderStageFlags::FRAGMENT) {
                        let slot = layout.res_overrides[&location_fs].resource_id;
                        inner.resources_fs.add_buffer(slot as _, &buffer, offset as _);
                        if let EncoderState::Render(ref encoder) = inner.encoder_state {
                            encoder.set_fragment_buffer(slot as _, offset as _, Some(buffer))
                        }
                    }
                }
            }
        }
    }

    fn bind_compute_pipeline(&mut self, pipeline: &native::ComputePipeline) {
        let inner = self.inner();
        inner.compute_pso = Some(pipeline.raw.to_owned());
        inner.work_group_size = pipeline.work_group_size;
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
                                }
                                Image(ref images) => {
                                    resources.add_textures(start, images.as_slice());
                                }
                                Buffer(ref buffers) => {
                                    for (i, ref bref) in buffers.iter().enumerate() {
                                        if let Some((ref buffer, offset)) = **bref {
                                            resources.add_buffer(start + i, buffer.as_ref(), offset as _);
                                        }
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
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        let inner = self.inner();
        let (encoder, wg_size) = inner.begin_compute();

        let group_counts = MTLSize {
            width: x as _,
            height: y as _,
            depth: z as _,
        };
        encoder.dispatch_thread_groups(group_counts, wg_size);

        encoder.end_encoding();
    }

    fn dispatch_indirect(&mut self, buffer: &native::Buffer, offset: u64) {
        let inner = self.inner();
        let (encoder, wg_size) = inner.begin_compute();

        encoder.dispatch_thread_groups_indirect(&buffer.raw, offset, wg_size);

        encoder.end_encoding();
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
        let encoder = self.encode_blit();

        for region in regions {
            let region = region.borrow();
            unsafe {
                msg_send![encoder,
                    copyFromBuffer: &*src.raw
                    sourceOffset: region.src as NSUInteger
                    toBuffer: &*dst.raw
                    destinationOffset: region.dst as NSUInteger
                    size: region.size as NSUInteger
                ]
            }
        }
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
        let encoder = self.encode_blit();
        let extent = MTLSize {
            width: dst.raw.width(),
            height: dst.raw.height(),
            depth: dst.raw.depth(),
        };
        // FIXME: layout

        for region in regions {
            let region = region.borrow();
            let image_offset = &region.image_offset;
            let r = &region.image_layers;

            for layer in r.layers.clone() {
                let row_pitch = div(region.buffer_width, dst.block_dim.0 as _) * dst.bytes_per_block as u32;
                let slice_pitch = div(region.buffer_height, dst.block_dim.1 as _) * row_pitch;

                let offset = region.buffer_offset + slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                unsafe {
                    msg_send![encoder,
                        copyFromBuffer: &*src.raw
                        sourceOffset: offset as NSUInteger
                        sourceBytesPerRow: row_pitch as NSUInteger
                        sourceBytesPerImage: slice_pitch as NSUInteger
                        sourceSize: extent
                        toTexture: &*dst.raw
                        destinationSlice: layer as NSUInteger
                        destinationLevel: r.level as NSUInteger
                        destinationOrigin: MTLOrigin { x: image_offset.x as _, y: image_offset.y as _, z: image_offset.z as _ }
                    ]
                }
            }
        }
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
        let encoder = self.encode_blit();
        let extent = MTLSize {
            width: src.raw.width(),
            height: src.raw.height(),
            depth: src.raw.depth(),
        };
        // FIXME: layout

        for region in regions {
            let region = region.borrow();
            let image_offset = &region.image_offset;
            let r = &region.image_layers;

            for layer in r.layers.clone() {
                let row_pitch = div(region.buffer_width, src.block_dim.0 as _) * src.bytes_per_block as u32;
                let slice_pitch = div(region.buffer_height, src.block_dim.1 as _) * row_pitch;

                let offset = region.buffer_offset + slice_pitch as NSUInteger * (layer - r.layers.start) as NSUInteger;
                unsafe {
                    msg_send![encoder,
                        copyFromTexture: &*src.raw
                        sourceSlice: layer as NSUInteger
                        sourceLevel: r.level as NSUInteger
                        sourceOrigin: MTLOrigin { x: image_offset.x as _, y: image_offset.y as _, z: image_offset.z as _ }
                        sourceSize: extent
                        toBuffer: &*dst.raw
                        destinationOffset: offset as NSUInteger
                        destinationBytesPerRow: row_pitch as NSUInteger
                        destinationBytesPerImage: slice_pitch as NSUInteger
                    ]
                }
            }
        }
    }

    fn draw(
        &mut self,
        vertices: Range<VertexCount>,
        instances: Range<InstanceCount>,
    ) {
        let primitive_type = self.inner().primitive_type;
        let encoder = self.expect_render_pass();

        unsafe {
            msg_send![encoder,
                drawPrimitives: primitive_type
                vertexStart: vertices.start as NSUInteger
                vertexCount: (vertices.end - vertices.start) as NSUInteger
                instanceCount: (instances.end - instances.start) as NSUInteger
                baseInstance: instances.start as NSUInteger
            ];
        }
    }

    fn draw_indexed(
        &mut self,
        indices: Range<IndexCount>,
        base_vertex: VertexOffset,
        instances: Range<InstanceCount>,
    ) {
        let (buffer, offset, index_type) = self.inner_ref().index_buffer.as_ref().cloned().expect("must bind index buffer");
        let primitive_type = self.inner_ref().primitive_type;
        let encoder = self.expect_render_pass();
        let index_offset = match index_type {
            MTLIndexType::UInt16 => indices.start as u64 * 2,
            MTLIndexType::UInt32 => indices.start as u64 * 4,
        };

        unsafe {
            msg_send![encoder,
                drawIndexedPrimitives: primitive_type
                indexCount: (indices.end - indices.start) as NSUInteger
                indexType: index_type
                indexBuffer: buffer
                indexBufferOffset: (index_offset + offset) as NSUInteger
                instanceCount: (instances.end - instances.start) as NSUInteger
                baseVertex: base_vertex as NSUInteger
                baseInstance: instances.start as NSUInteger
            ];
        }
    }

    fn draw_indirect(
        &mut self,
        _buffer: &native::Buffer,
        _offset: u64,
        _draw_count: u32,
        _stride: u32,
    ) {
        unimplemented!()
    }

    fn draw_indexed_indirect(
        &mut self,
        _buffer: &native::Buffer,
        _offset: u64,
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
