//! Commands encoder.

use std::mem;
use std::ops::Range;
use std::sync::mpsc;
use std::collections::{HashMap, HashSet};

use hal::{self, buffer as b, image as i, CommandPool};
use hal::command::CommandBuffer;
use hal::format::AspectFlags;
use hal::memory::Barrier;
use hal::pso::PipelineStage;

use memory::{Provider, Dependency, cast_slice};
use device::InitToken;
use {handle, buffer, image, format, pso};
use {Backend, Supports, Transfer, Graphics};
use {VertexCount};

pub use hal::command::{
    BufferCopy, ImageCopy, BufferImageCopy,
    ClearColor, ClearDepthStencil,
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
            buffer: self.mut_inner().acquire_command_buffer(false),
            // raw_data: pso::RawDataSet::new(),
            handles: handle::Bag::new(),
            pipeline_stage: PipelineStage::TOP_OF_PIPE,
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
    pipeline_stage: PipelineStage,
    buffer_states: HashMap<handle::raw::Buffer<B>, hal::buffer::State>,
    image_states: HashMap<handle::raw::Image<B>, ImageStates>,
}

pub struct Submit<B: Backend, C> {
    pub(crate) inner: hal::command::Submit<B, C, hal::command::OneShot, hal::command::Primary>,
    pub(crate) access_info: AccessInfo<B>,
    pub(crate) handles: handle::Bag<B>,
    pub(crate) pool: PoolDependency<B, C>
}

// TODO: coalescing?
struct ImageStates {
    states: Vec<hal::image::State>,
    levels: usize,
}

impl ImageStates {
    fn new(
        state: hal::image::State,
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
    ) -> &mut hal::image::State {
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
        self.buffer.pipeline_barrier(stage_transition, barriers);
    }

    fn init_image<'b>(
        &mut self, image: &'b handle::raw::Image<B>
    ) -> Barrier<'b, B> {
        let creation_state = (hal::image::Access::empty(), i::ImageLayout::Undefined);
        let num_levels = image.info().mip_levels;
        let num_layers = image.info().kind.num_layers();
        let stable_state = image.info().stable_state;
        let states = ImageStates::new(stable_state, num_levels, num_layers);
        self.image_states.insert(image.clone(), states);
        Barrier::Image {
            states: creation_state .. stable_state,
            target: image.resource(),
            range: i::SubresourceRange {
                aspects: image.info().aspects,
                levels: 0 .. num_levels,
                layers: 0 .. num_layers,
            },
        }
    }

    fn require_buffer_state<'b>(
        &mut self,
        buffer: &'b handle::raw::Buffer<B>,
        state: hal::buffer::State
    ) -> Option<Barrier<'b, B>> {
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
        state: hal::image::State,
    ) -> Option<Barrier<'b, B>> {
        if !self.image_states.contains_key(image) {
            let levels = image.info().mip_levels;
            let layers = image.info().kind.num_layers();
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
        current: &mut hal::buffer::State,
        next: hal::buffer::State
    ) -> Option<Barrier<'b, B>> {
        let state = mem::replace(current, next);
        if state != next {
            Some(Barrier::Buffer {
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
        current: &mut hal::image::State,
        next: hal::image::State,
    ) -> Option<Barrier<'b, B>> {
        let state = mem::replace(current, next);
        if state != next {
            Some(Barrier::Image {
                states: state .. next,
                target: image.resource(),
                range: i::SubresourceRange {
                    aspects: image.info().aspects,
                    levels: level .. (level+1),
                    layers: layer .. (layer+1),
                },
            })
        } else {
            None
        }
    }

    #[doc(hidden)]
    pub fn require_state(
        &mut self,
        stage: hal::pso::PipelineStage,
        buffer_states: &[(&handle::raw::Buffer<B>, hal::buffer::State)],
        image_states: &[(&handle::raw::Image<B>, image::Subresource, hal::image::State)],
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
            self.buffer.pipeline_barrier(current_stage..stage, barriers);
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
        let stage_transition = self.pipeline_stage..PipelineStage::BOTTOM_OF_PIPE;
        self.buffer.pipeline_barrier(stage_transition, barriers);
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
        let src = src.as_ref();
        let dst = dst.as_ref();

        debug_assert!(src.info().usage.contains(b::Usage::TRANSFER_SRC),
            "missing TRANSFER_SRC usage flag");
        debug_assert!(dst.info().usage.contains(b::Usage::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        let stride = mem::size_of::<MTB::Data>() as u64;
        let byte_regions = regions.iter()
            .map(|region| BufferCopy {
                src: region.src * stride,
                dst: region.dst * stride,
                size: region.size * stride,
            });

        // TODO: check alignement
        // TODO: check copy_buffer capability
        if cfg!(debug) {
            let mut byte_regions: Vec<_> = byte_regions.collect();
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
            self.require_state(
                PipelineStage::TRANSFER,
                &[
                    (src, b::Access::TRANSFER_READ),
                    (dst, b::Access::TRANSFER_WRITE)
                ],
                &[]);

            self.buffer.copy_buffer(
                src.resource(),
                dst.resource(),
                byte_regions);
        } else {
            self.require_state(
                PipelineStage::TRANSFER,
                &[
                    (src, b::Access::TRANSFER_READ),
                    (dst, b::Access::TRANSFER_WRITE)
                ],
                &[]);

            self.buffer.copy_buffer(
                src.resource(),
                dst.resource(),
                byte_regions);
        }
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
        let buffer = buffer.as_ref();

        debug_assert!(buffer.info().usage.contains(b::Usage::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        let stride = mem::size_of::<MTB::Data>() as u64;
        let start_bytes = offset * stride;
        let end_bytes = start_bytes + data.len() as u64 * stride;
        debug_assert!(end_bytes <= buffer.info().size,
            "out of buffer bounds");

        self.require_state(
            PipelineStage::TRANSFER,
            &[(buffer, b::Access::TRANSFER_WRITE)],
            &[]);

        self.buffer.update_buffer(
            buffer.resource(),
            start_bytes,
            cast_slice(data));
    }

    pub fn copy_image<IA, IB>(
        &mut self,
        src: IA,
        dst: IB,
        regions: &[ImageCopy],
    )
        where IA: AsRef<handle::raw::Image<B>>,
              IB: AsRef<handle::raw::Image<B>>
    {
        let src = src.as_ref();
        let dst = dst.as_ref();
        if regions.is_empty() { return };

        debug_assert!(src.info().usage.contains(i::Usage::TRANSFER_SRC),
            "missing TRANSFER_SRC usage flag");
        debug_assert!(dst.info().usage.contains(i::Usage::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        // TODO: error handling
        let src_state = (i::Access::TRANSFER_READ, i::ImageLayout::TransferSrcOptimal);
        let dst_state = (i::Access::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal);
        let mut image_states = Vec::new();
        for region in regions {
            image_states.push((src, region.src_subresource, src_state));
            image_states.push((dst, region.dst_subresource, dst_state));
        }
        self.require_state(
            PipelineStage::TRANSFER,
            &[],
            &image_states[..]);

        self.buffer.copy_image(
            src.resource(), src_state.1,
            dst.resource(), dst_state.1,
            regions);
    }

    /// Copy part of a buffer to an image
    pub fn copy_buffer_to_image<BA, IB>(
        &mut self,
        src: BA,
        dst: IB,
        regions: &[BufferImageCopy],
    )
        where BA: AsRef<handle::raw::Buffer<B>>,
              IB: AsRef<handle::raw::Image<B>>
    {
        let src = src.as_ref();
        let dst = dst.as_ref();
        if regions.is_empty() { return };

        debug_assert!(src.info().usage.contains(b::Usage::TRANSFER_SRC),
            "missing TRANSFER_SRC usage flag");
        debug_assert!(dst.info().usage.contains(i::Usage::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        // TODO: error handling
        let dst_state = (i::Access::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal);
        let mut image_states = Vec::new();
        for region in regions {
            let r = &region.image_layers;
            for layer in r.layers.clone() {
                image_states.push((dst, (r.level, layer), dst_state));
            }
        }
        self.require_state(
            PipelineStage::TRANSFER,
            &[(src, b::Access::TRANSFER_READ)],
            &image_states[..]);

        self.buffer.copy_buffer_to_image(
            src.resource(),
            dst.resource(), dst_state.1,
            regions);
    }

    /// Copy part of an image to a buffer
    pub fn copy_image_to_buffer<IA, BB>(
        &mut self,
        src: IA,
        dst: BB,
        regions: &[BufferImageCopy],
    )
        where IA: AsRef<handle::raw::Image<B>>,
              BB: AsRef<handle::raw::Buffer<B>>
    {
        let src = src.as_ref();
        let dst = dst.as_ref();
        if regions.is_empty() { return };

        debug_assert!(src.info().usage.contains(i::Usage::TRANSFER_SRC),
            "missing TRANSFER_SRC usage flag");
        debug_assert!(dst.info().usage.contains(b::Usage::TRANSFER_DST),
            "missing TRANSFER_DST usage flag");

        // TODO: error handling
        let src_state = (i::Access::TRANSFER_READ, i::ImageLayout::TransferSrcOptimal);
        let mut image_states = Vec::new();
        for region in regions {
            let r = &region.image_layers;
            for layer in r.layers.clone() {
                image_states.push((src, (r.level, layer), src_state));
            }
        }
        self.require_state(
            PipelineStage::TRANSFER,
            &[(dst, b::Access::TRANSFER_WRITE)],
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
    fn require_clear_state(&mut self, image: &handle::raw::Image<B>) -> i::ImageLayout {
        let levels = image.info().mip_levels;
        let layers = image.info().kind.num_layers();
        let state = (i::Access::TRANSFER_WRITE, i::ImageLayout::TransferDstOptimal);
        let mut image_states = Vec::new();
        for level in 0..levels {
            for layer in 0..layers {
                image_states.push((image, (level, layer), state));
            }
        }
        self.require_state(
            PipelineStage::TRANSFER,
            &[],
            &image_states[..]);

        state.1
    }

    /// Clears `image` to `value`.
    pub fn clear_color_raw(
        &mut self,
        image: &handle::raw::Image<B>,
        value: ClearColor,
    ) {
        let layout = self.require_clear_state(image);
        //TODO
        let range = i::SubresourceRange {
            aspects: AspectFlags::COLOR,
            levels: 0 .. 1,
            layers: 0 .. 1,
        };
        self.handles.add(image.clone());
        self.buffer.clear_color_image(image.resource(), layout, range, value);
    }

    /// Clears `image` to `value`.
    pub fn clear_color<F>(
        &mut self,
        image: &handle::Image<B, F>,
        value: ClearColor,
    ) where
        F: format::AsFormat,
    {
        self.clear_color_raw(image.as_ref(), value);
    }

    /// Clears `image`'s depth/stencil with `value`
    pub fn clear_depth_stencil_raw(
        &mut self,
        image: &handle::raw::Image<B>,
        value: ClearDepthStencil,
    ) {
        let layout = self.require_clear_state(image);
        //TODO
        let range = i::SubresourceRange {
            aspects: AspectFlags::DEPTH | AspectFlags::STENCIL,
            levels: 0 .. 1,
            layers: 0 .. 1,
        };
        self.handles.add(image.clone());
        self.buffer.clear_depth_stencil_image(image.resource(), layout, range, value);
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
