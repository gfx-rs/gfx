//! Commands encoder.

use std::mem;
use std::ops::Range;
use std::sync::mpsc;
use std::collections::{HashMap, HashSet};

use core::{self, CommandPool};
use core::command::CommandBuffer;
use core::image::ImageLayout;
use memory::{Provider, Dependency, cast_slice};
use device::InitToken;
use {handle, buffer, image, format, pso};
use {Backend, Supports, Transfer, Graphics};
use {VertexCount};

pub use core::command::{
    BufferCopy, ImageCopy, BufferImageCopy,
    ClearColor
};

pub struct Pool<B: Backend, C>(Provider<PoolInner<B, C>>);

#[derive(Clone)]
pub(crate) struct PoolDependency<B: Backend, C>(Dependency<PoolInner<B, C>>);

impl<B: Backend, C> Pool<B, C> {
    pub(crate) fn new(
        inner: CommandPool<B, C>,
        sender: CommandPoolSender<B, C>
    ) -> Self {
        Pool(Provider::new(PoolInner { inner: Some(inner), sender }))
    }

    fn mut_inner<'a>(&'a mut self) -> &'a mut CommandPool<B, C> {
        self.0.get_mut()
    }

    pub fn reserve(&mut self, additional: usize) {
        self.mut_inner().reserve(additional);
    }

    pub fn acquire_encoder<'a>(&'a mut self) -> Encoder<'a, B, C> {
        Encoder {
            pool: PoolDependency(self.0.dependency()),
            buffer: self.mut_inner().acquire_command_buffer(),
            // raw_data: pso::RawDataSet::new(),
            handles: handle::Bag::new(),
            pipeline_stage: core::pso::TOP_OF_PIPE,
            buffer_states: HashMap::new(),
            image_states: HashMap::new(),
        }
    }
}

struct PoolInner<B: Backend, C> {
    // option for owned drop
    inner: Option<CommandPool<B, C>>,
    sender: CommandPoolSender<B, C>,
}

impl<B: Backend, C> PoolInner<B, C> {
    fn get_mut(&mut self) -> &mut CommandPool<B, C> {
        self.inner.as_mut().unwrap()
    }
}

impl<B: Backend, C> Drop for PoolInner<B, C> {
    fn drop(&mut self) {
        // simply will not be recycled if the channel is down, should be ok.
        let _ = self.sender.send(self.inner.take().unwrap());
    }
}

pub(crate) type CommandPoolSender<B, C> = mpsc::Sender<CommandPool<B, C>>;
pub(crate) type CommandPoolReceiver<B, C> = mpsc::Receiver<CommandPool<B, C>>;
pub(crate) fn command_pool_channel<B: Backend, C>()
    -> (CommandPoolSender<B, C>, CommandPoolReceiver<B, C>) {
    mpsc::channel()
}

pub struct Encoder<'a, B: Backend, C> {
    buffer: CommandBuffer<'a, B, C>,
    handles: handle::Bag<B>,
    pool: PoolDependency<B, C>,
    // raw_data: pso::RawDataSet<B>,
    pipeline_stage: core::pso::PipelineStage,
    buffer_states: HashMap<handle::raw::Buffer<B>, core::buffer::State>,
    image_states: HashMap<handle::raw::Image<B>, ImageStates>,
}

pub struct Submit<B: Backend, C> {
    pub(crate) inner: core::command::Submit<B, C>,
    pub(crate) access_info: AccessInfo<B>,
    pub(crate) handles: handle::Bag<B>,
    pub(crate) pool: PoolDependency<B, C>
}

// TODO: coalescing?
struct ImageStates {
    states: Vec<core::image::State>,
    levels: usize,
}

impl ImageStates {
    fn new(
        state: core::image::State,
        levels: image::Level,
        layers: image::Layer,
    ) -> Self {
        let size = layers as usize * levels as usize;
        ImageStates {
            states: (0..size).map(|_| state).collect(),
            levels: levels as usize,
        }
    }

    fn get_mut(
        &mut self,
        level: image::Level,
        layer: image::Layer,
    ) -> &mut core::image::State {
        let index = layer as usize * self.levels + level as usize;
        &mut self.states[index]
    }

    fn ranges(&self) -> (Range<image::Level>, Range<image::Layer>) {
        let levels = self.levels as image::Level;
        let layers = (self.states.len() / self.levels) as image::Layer;
        (0..levels, 0..layers)
    }
}

/// Informations about what is accessed by a submit.
#[derive(Debug)]
pub struct AccessInfo<B: Backend> {
    buffers: HashSet<handle::raw::Buffer<B>>,
}

impl<B: Backend> AccessInfo<B> {
    /// Creates empty access informations
    pub fn new() -> Self {
        AccessInfo {
            buffers: HashSet::new(),
        }
    }

    pub fn from_map<T>(map: HashMap<handle::raw::Buffer<B>, T>) -> Self {
        AccessInfo {
            buffers: map.into_iter().map(|(handle, _)| handle).collect()
        }
    }

    /// Clear access informations
    pub fn clear(&mut self) {
        self.buffers.clear();
    }

    pub fn append(&mut self, other: &mut AccessInfo<B>) {
        self.buffers.extend(other.buffers.drain());
    }

    pub(crate) fn start_gpu_access(&self) {
        let accesses = self.buffers.iter()
            .map(|buffer| &buffer.info().access);

        for access in accesses {
            assert!(access.acquire_cpu(), "access overlap on submission");
            access.gpu_start();
            access.release_cpu();
        }
    }

    pub(crate) fn end_gpu_access(&self) {
        for buffer in &self.buffers {
            buffer.info().access.gpu_end()
        }
    }
}

impl<'a, B: Backend, C> Encoder<'a, B, C> {
    pub fn mut_buffer(&mut self) -> &mut CommandBuffer<'a, B, C> {
        &mut self.buffer
    }
}

impl<'a, B: Backend, C> Encoder<'a, B, C>
    where C: Supports<Transfer>
{
    pub fn finish(mut self) -> Submit<B, C> {
        self.transition_to_stable_state();
        self.handles.extend(self.image_states.into_iter().map(|kv| kv.0));
        Submit {
            inner: self.buffer.finish(),
            access_info: AccessInfo::from_map(self.buffer_states),
            handles: self.handles,
            pool: self.pool,
        }
    }

    pub fn init_resources(&mut self, tokens: Vec<InitToken<B>>) {
        let mut barriers = Vec::new();
        for token in &tokens {
            match token.handle {
                handle::Any::Image(ref image) =>
                    barriers.push(self.init_image(image)),
                _ => {}
            }
        }
        let stage_transition = self.pipeline_stage..self.pipeline_stage;
        self.buffer.pipeline_barrier(stage_transition, &barriers[..]);
    }

    fn init_image<'b>(&mut self, image: &'b handle::raw::Image<B>)
        -> core::memory::Barrier<'b, B>
    {
        let creation_state = (core::image::Access::empty(), ImageLayout::Undefined);
        let levels = image.info().mip_levels;
        let layers = image.info().kind.get_num_layers();
        let stable_state = image.info().stable_state;
        let states = ImageStates::new(stable_state, levels, layers);
        self.image_states.insert(image.clone(), states);
        core::memory::Barrier::Image {
            states: creation_state..stable_state,
            target: image.resource(),
            range: (0..levels, 0..layers)
        }
    }

    fn require_buffer_state<'b>(
        &mut self,
        buffer: &'b handle::raw::Buffer<B>,
        state: core::buffer::State
    ) -> Option<core::memory::Barrier<'b, B>> {
        if !self.buffer_states.contains_key(buffer) {
            self.buffer_states.insert(buffer.clone(), buffer.info().stable_state);
        }
        Self::transition_buffer(
            buffer,
            self.buffer_states.get_mut(buffer).unwrap(),
            state)
    }

    fn require_image_state<'b>(
        &mut self,
        image: &'b handle::raw::Image<B>,
        level: image::Level,
        layer: image::Layer,
        state: core::image::State,
    ) -> Option<core::memory::Barrier<'b, B>> {
        if !self.image_states.contains_key(image) {
            let levels = image.info().mip_levels;
            let layers = image.info().kind.get_num_layers();
            let states = ImageStates::new(image.info().stable_state, levels, layers);
            self.image_states.insert(image.clone(), states);
        }
        Self::transition_image(
            image,
            level,
            layer,
            self.image_states.get_mut(image).unwrap().get_mut(level, layer),
            state)
    }

    fn transition_buffer<'b>(
        buffer: &'b handle::raw::Buffer<B>,
        current: &mut core::buffer::State,
        next: core::buffer::State
    ) -> Option<core::memory::Barrier<'b, B>> {
        let state = mem::replace(current, next);
        if state != next {
            Some(core::memory::Barrier::Buffer {
                states: state..next,
                target: buffer.resource(),
            })
        } else {
            None
        }
    }

    fn transition_image<'b>(
        image: &'b handle::raw::Image<B>,
        level: image::Level,
        layer: image::Layer,
        current: &mut core::image::State,
        next: core::image::State,
    ) -> Option<core::memory::Barrier<'b, B>> {
        let state = mem::replace(current, next);
        if state != next {
            Some(core::memory::Barrier::Image {
                states: state..next,
                target: image.resource(),
                range: (level..(level+1), layer..(layer+1)),
            })
        } else {
            None
        }
    }

    #[doc(hidden)]
    pub fn require_state(
        &mut self,
        stage: core::pso::PipelineStage,
        buffer_states: &[(&handle::raw::Buffer<B>, core::buffer::State)],
        image_states: &[(&handle::raw::Image<B>, image::Subresource, core::image::State)],
    ) {
        let mut barriers = Vec::new();
        for &(buffer, state) in buffer_states {
            barriers.extend(self.require_buffer_state(buffer, state));
        }
        for &(image, (level, layer), state) in image_states {
            barriers.extend(self.require_image_state(image, level, layer, state));
        }
        let current_stage = mem::replace(&mut self.pipeline_stage, stage);
        if (current_stage != stage) || !barriers.is_empty() {
            self.buffer.pipeline_barrier(current_stage..stage, &barriers[..]);
        }
    }

    #[doc(hidden)]
    pub fn handles(&mut self) -> &mut handle::Bag<B> {
        &mut self.handles
    }

    fn transition_to_stable_state(&mut self) {
        let mut barriers = Vec::new();
        for (buffer, state) in &mut self.buffer_states {
            barriers.extend(Self::transition_buffer(buffer, state, buffer.info().stable_state));
        }
        for (image, states) in &mut self.image_states {
            let (levels, layers) = states.ranges();
            for level in levels {
                for layer in layers.clone() {
                    let state = states.get_mut(level, layer);
                    barriers.extend(Self::transition_image(image, level, layer, state, image.info().stable_state));
                }
            }
        }
        let stage_transition = self.pipeline_stage..core::pso::BOTTOM_OF_PIPE;
        self.buffer.pipeline_barrier(stage_transition, &barriers[..]);
    }

    // TODO: fill buffer

    /// Copy part of a buffer to another
    pub fn copy_buffer<MTB>(
        &mut self,
        src: &MTB,
        dst: &MTB,
        regions: &[BufferCopy],
    )
        where MTB: buffer::MaybeTyped<B>
    {
        if regions.is_empty() { return };
        let src = src.as_raw();
        let dst = dst.as_raw();

        debug_assert!(src.info().usage.contains(buffer::TRANSFER_SRC),
            "missing TRANSFER_SRC usage flag");
        debug_assert!(dst.info().usage.contains(buffer::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        let stride = mem::size_of::<MTB::Data>() as u64;
        let mut byte_regions: Vec<_> = regions.iter()
            .map(|region| BufferCopy {
                src: region.src * stride,
                dst: region.dst * stride,
                size: region.size * stride,
            }).collect();

        // TODO: check alignement
        // TODO: check copy_buffer capability
        if cfg!(debug) {
            byte_regions.sort_by(|a, b| a.src.cmp(&b.src));
            let mut src_range = None;
            for &BufferCopy { src, size, .. } in &byte_regions {
                if let Some((_, ref mut end)) = src_range {
                    assert!(src > *end, "source region overlap");
                    *end = src + size;
                } else {
                    src_range = Some((src, src + size));
                }
            }
            let src_range = src_range.unwrap();

            byte_regions.sort_by(|a, b| a.dst.cmp(&b.dst));
            let mut dst_range = None;
            for &BufferCopy { dst, size, .. } in &byte_regions {
                if let Some((_, ref mut end)) = dst_range {
                    assert!(dst > *end, "destination region overlap");
                    *end = dst + size;
                } else {
                    dst_range = Some((dst, dst + size));
                }
            }
            let dst_range = dst_range.unwrap();

            assert!(src_range.1 <= src.info().size, "out of source bounds");
            assert!(dst_range.1 <= dst.info().size, "out of destination bounds");
            // TODO: check if src == dst
        }

        self.require_state(
            core::pso::TRANSFER,
            &[(src, core::buffer::TRANSFER_READ),
               (dst, core::buffer::TRANSFER_WRITE)],
            &[]);

        self.buffer.copy_buffer(
            src.resource(),
            dst.resource(),
            &byte_regions[..]);
    }

    /// Update a buffer with a slice of data.
    pub fn update_buffer<MTB>(
        &mut self,
        buffer: &MTB,
        offset: u64,
        data: &[MTB::Data],
    )
        where MTB: buffer::MaybeTyped<B>
    {
        if data.is_empty() { return; }
        let buffer = buffer.as_raw();

        debug_assert!(buffer.info().usage.contains(buffer::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        let stride = mem::size_of::<MTB::Data>() as u64;
        let start_bytes = offset * stride;
        let end_bytes = start_bytes + data.len() as u64 * stride;
        debug_assert!(end_bytes <= buffer.info().size,
            "out of buffer bounds");

        self.require_state(
            core::pso::TRANSFER,
            &[(buffer, core::buffer::TRANSFER_WRITE)],
            &[]);
            
        self.buffer.update_buffer(
            buffer.resource(),
            start_bytes,
            cast_slice(data));
    }

    pub fn copy_image(
        &mut self,
        src: &handle::raw::Image<B>,
        dst: &handle::raw::Image<B>,
        regions: &[ImageCopy],
    ) {
        if regions.is_empty() { return };

        debug_assert!(src.info().usage.contains(image::TRANSFER_SRC),
            "missing TRANSFER_SRC usage flag");
        debug_assert!(dst.info().usage.contains(image::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        // TODO: error handling
        let src_state = (core::image::TRANSFER_READ, ImageLayout::TransferSrcOptimal);
        let dst_state = (core::image::TRANSFER_WRITE, ImageLayout::TransferDstOptimal);
        let mut image_states = Vec::new();
        for region in regions {
            image_states.push((src, region.src_subresource, src_state));
            image_states.push((dst, region.dst_subresource, dst_state));
        }
        self.require_state(
            core::pso::TRANSFER,
            &[],
            &image_states[..]);

        self.buffer.copy_image(
            src.resource(), src_state.1,
            dst.resource(), dst_state.1,
            regions);
    }
    
    /// Copy part of a buffer to an image
    pub fn copy_buffer_to_image(
        &mut self,
        src: &handle::raw::Buffer<B>,
        dst: &handle::raw::Image<B>,
        regions: &[BufferImageCopy],
    ) {
        if regions.is_empty() { return };

        debug_assert!(src.info().usage.contains(buffer::TRANSFER_SRC),
            "missing TRANSFER_SRC usage flag");
        debug_assert!(dst.info().usage.contains(image::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        // TODO: error handling
        let dst_state = (core::image::TRANSFER_WRITE, ImageLayout::TransferDstOptimal);
        let mut image_states = Vec::new();
        for region in regions {
            let (level, ref layers) = region.image_subresource;
            for layer in layers.clone() {
                image_states.push((dst, (level, layer), dst_state));
            }
        }
        self.require_state(
            core::pso::TRANSFER,
            &[(src, core::buffer::TRANSFER_READ)],
            &image_states[..]);

        self.buffer.copy_buffer_to_image(
            src.resource(),
            dst.resource(), dst_state.1,
            regions);
    }

    /// Copy part of an image to a buffer
    pub fn copy_image_to_buffer(
        &mut self,
        src: &handle::raw::Image<B>,
        dst: &handle::raw::Buffer<B>,
        regions: &[BufferImageCopy],
    ) {
        if regions.is_empty() { return };

        debug_assert!(src.info().usage.contains(image::TRANSFER_SRC),
            "missing TRANSFER_SRC usage flag");
        debug_assert!(dst.info().usage.contains(buffer::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        // TODO: error handling
        let src_state = (core::image::TRANSFER_READ, ImageLayout::TransferSrcOptimal);
        let mut image_states = Vec::new();
        for region in regions {
            let (level, ref layers) = region.image_subresource;
            for layer in layers.clone() {
                image_states.push((src, (level, layer), src_state));
            }
        }
        self.require_state(
            core::pso::TRANSFER,
            &[(dst, core::buffer::TRANSFER_WRITE)],
            &image_states[..]);

        self.buffer.copy_image_to_buffer(
            src.resource(), src_state.1,
            dst.resource(),
            regions);
    }
}

impl<'a, B: Backend, C> Encoder<'a, B, C>
    where C: Supports<Transfer> + Supports<Graphics>
{
    fn require_clear_state(&mut self, image: &handle::raw::Image<B>) -> ImageLayout {
        let levels = image.info().mip_levels;
        let layers = image.info().kind.get_num_layers();
        let state = (core::image::TRANSFER_WRITE, ImageLayout::TransferDstOptimal);
        let mut image_states = Vec::new();
        for level in 0..levels {
            for layer in 0..layers {
                image_states.push((image, (level, layer), state));
            }
        }
        self.require_state(
            core::pso::TRANSFER,
            &[],
            &image_states[..]);

        state.1
    }

    /// Clears `rtv` to `value`.
    pub fn clear_color_raw(
        &mut self,
        rtv: &handle::raw::RenderTargetView<B>,
        value: ClearColor,
    ) {
        let layout = self.require_clear_state(rtv.info());
        self.handles.add(rtv.clone());
        self.buffer.clear_color(rtv.resource(), layout, value);
    }

    /// Clears `rtv` to `value`.
    pub fn clear_color<F>(
        &mut self,
        rtv: &handle::RenderTargetView<B, F>,
        value: F::View
    )
        where F: format::RenderFormat, F::View: Into<ClearColor>
    {
        self.clear_color_raw(rtv, value.into());
    }

    /// Clears `dsv`'s depth to `depth_value` and stencil to `stencil_value`, if some.
    pub fn clear_depth_stencil_raw(
        &mut self,
        dsv: &handle::raw::DepthStencilView<B>,
        depth_value: Option<core::target::Depth>,
        stencil_value: Option<core::target::Stencil>
    ) {
        let layout = self.require_clear_state(dsv.info());
        self.handles.add(dsv.clone());
        self.buffer.clear_depth_stencil(dsv.resource(), layout, depth_value, stencil_value);
    }

    pub fn draw<D>(
        &mut self,
        vertices: Range<VertexCount>,
        pipeline: &D::Pipeline,
        data: D
    )
        where D: pso::GraphicsPipelineData<B>
    {
        // TODO: instances
        data.begin_renderpass(self, pipeline).draw(vertices, 0..1);
    }

/*
    /// Generate a mipmap chain for the given resource view.
    pub fn generate_mipmap<T: format::BlendFormat>(&mut self, view: &handle::ShaderResourceView<B, T>) {
        self.generate_mipmap_raw(view.raw())
    }

    /// Untyped version of mipmap generation.
    pub fn generate_mipmap_raw(&mut self, view: &handle::RawShaderResourceView<B>) {
        let srv = self.handles.ref_srv(view).clone();
        self.command_buffer.generate_mipmap(srv);
    }
    */
}
