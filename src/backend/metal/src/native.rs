use {AsNative, Backend, ResourceIndex, BufferPtr, SamplerPtr, TexturePtr};
use internal::{Channel, FastStorageMap};
use range_alloc::RangeAllocator;
use window::SwapchainImage;

use std::borrow::Borrow;
use std::cell::RefCell;
use std::{fmt, iter};
use std::ops::Range;
use std::os::raw::{c_void, c_long};
use std::sync::Arc;

use hal::{buffer, image, pso};
use hal::{DescriptorPool as HalDescriptorPool, MemoryTypeId};
use hal::backend::FastHashMap;
use hal::command::{ClearColorRaw, ClearValueRaw};
use hal::format::{Aspects, Format, FormatDesc};
use hal::pass::{Attachment, AttachmentLoadOp, AttachmentOps};
use hal::range::RangeArg;

use cocoa::foundation::{NSUInteger};
use metal;
use parking_lot::{Mutex, RwLock};
use smallvec::SmallVec;
use spirv_cross::{msl, spirv};


pub type EntryPointMap = FastHashMap<String, spirv::EntryPoint>;
/// An index of a resource within descriptor pool.
pub type PoolResourceIndex = u32;

/// Shader module can be compiled in advance if it's resource bindings do not
/// depend on pipeline layout, in which case the value would become `Compiled`.
pub enum ShaderModule {
    Compiled(ModuleInfo),
    Raw(Vec<u8>),
}

impl fmt::Debug for ShaderModule {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ShaderModule::Compiled(_) => {
                write!(formatter, "ShaderModule::Compiled(..)")
            }
            ShaderModule::Raw(ref vec) => {
                write!(formatter, "ShaderModule::Raw(length = {})", vec.len())
            }
        }
    }
}

unsafe impl Send for ShaderModule {}
unsafe impl Sync for ShaderModule {}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct RenderPassKey {
    // enough room for 4 color targets + depth/stencil
    pub clear_data: SmallVec<[u32; 20]>,
    operations: SmallVec<[AttachmentOps; 6]>,
}

#[derive(Debug)]
pub struct RenderPass {
    pub(crate) attachments: Vec<Attachment>,
}

unsafe impl Send for RenderPass {}
unsafe impl Sync for RenderPass {}

impl RenderPass {
    pub fn build_key<T>(&self, clear_values: T) -> (RenderPassKey, Aspects)
    where
        T: IntoIterator,
        T::Item: Borrow<ClearValueRaw>,
    {
        let mut key = RenderPassKey::default();
        let mut full_aspects = Aspects::empty();

        let dummy_value = ClearValueRaw {
            color: ClearColorRaw {
                int32: [0; 4],
            },
        };
        let clear_values_iter = clear_values
            .into_iter()
            .map(|c| *c.borrow())
            .chain(iter::repeat(dummy_value));

        for (rat, clear_value) in self.attachments.iter().zip(clear_values_iter) {
            //TODO: avoid calling `surface_desc` as often
            let aspects = match rat.format {
                Some(format) => format.surface_desc().aspects,
                None => continue,
            };
            full_aspects |= aspects;
            let cv = clear_value.borrow();

            if aspects.contains(Aspects::COLOR) {
                key.operations.push(rat.ops);
                if rat.ops.load == AttachmentLoadOp::Clear {
                    key.clear_data.extend_from_slice(unsafe { &cv.color.uint32 });
                }
            }
            if aspects.contains(Aspects::DEPTH) {
                key.operations.push(rat.ops);
                if rat.ops.load == AttachmentLoadOp::Clear {
                    key.clear_data.push(unsafe { *(&cv.depth_stencil.depth as *const _ as *const u32) });
                }
            }
            if aspects.contains(Aspects::STENCIL) {
                key.operations.push(rat.stencil_ops);
                if rat.stencil_ops.load == AttachmentLoadOp::Clear {
                    key.clear_data.push(unsafe { cv.depth_stencil.stencil });
                }
            }
        }

        (key, full_aspects)
    }
}

#[derive(Clone, Debug)]
pub struct ColorAttachment {
    pub mtl_format: metal::MTLPixelFormat,
    pub channel: Channel,
}

#[derive(Clone, Debug)]
pub struct FramebufferInner {
    pub extent: image::Extent,
    pub aspects: Aspects,
    pub colors: SmallVec<[ColorAttachment; 4]>,
    pub depth_stencil: Option<metal::MTLPixelFormat>,
}

#[derive(Debug)]
pub struct Framebuffer {
    pub(crate) descriptor: metal::RenderPassDescriptor,
    pub(crate) desc_storage: FastStorageMap<RenderPassKey, metal::RenderPassDescriptor>,
    pub(crate) inner: FramebufferInner,
}

unsafe impl Send for Framebuffer {}
unsafe impl Sync for Framebuffer {}


#[derive(Clone, Debug)]
pub struct ResourceData<T> {
    pub buffers: T,
    pub textures: T,
    pub samplers: T,
}

impl<T> ResourceData<T> {
    pub fn map<V, F: Fn(&T) -> V>(&self, fun: F) -> ResourceData<V> {
        ResourceData {
            buffers: fun(&self.buffers),
            textures: fun(&self.textures),
            samplers: fun(&self.samplers),
        }
    }
}

impl ResourceData<PoolResourceIndex> {
    pub fn new() -> Self {
        ResourceData {
            buffers: 0,
            textures: 0,
            samplers: 0,
        }
    }
}
/*
impl ResourceData<ResourceIndex> {
    pub fn new() -> Self {
        ResourceCounters {
            buffers: 0,
            textures: 0,
            samplers: 0,
        }
    }
}
*/
impl ResourceData<PoolResourceIndex> {
    #[inline]
    pub fn add_many(&mut self, content: DescriptorContent, count: PoolResourceIndex) {
        if content.contains(DescriptorContent::BUFFER) {
            self.buffers += count;
        }
        if content.contains(DescriptorContent::TEXTURE) {
            self.textures += count;
        }
        if content.contains(DescriptorContent::SAMPLER) {
            self.samplers += count;
        }
    }
    #[inline]
    pub fn add(&mut self, content: DescriptorContent) {
        self.add_many(content, 1)
    }
}


#[derive(Clone, Debug)]
pub struct MultiStageData<T> {
    pub vs: T,
    pub ps: T,
    pub cs: T,
}

pub type MultiStageResourceCounters = MultiStageData<ResourceData<ResourceIndex>>;

impl MultiStageResourceCounters {
    pub fn add(&mut self, stages: pso::ShaderStageFlags, content: DescriptorContent) {
        if stages.contains(pso::ShaderStageFlags::VERTEX) {
            self.vs.add(content);
        }
        if stages.contains(pso::ShaderStageFlags::FRAGMENT) {
            self.ps.add(content);
        }
        if stages.contains(pso::ShaderStageFlags::COMPUTE) {
            self.cs.add(content);
        }
    }
}

#[derive(Debug)]
pub struct DescriptorSetInfo {
    pub offsets: MultiStageResourceCounters,
    pub dynamic_buffers: Vec<MultiStageData<PoolResourceIndex>>,
}

#[derive(Debug)]
pub struct PipelineLayout {
    pub(crate) shader_compiler_options: msl::CompilerOptions,
    pub(crate) shader_compiler_options_point: msl::CompilerOptions,
    pub(crate) infos: Vec<DescriptorSetInfo>,
    pub(crate) total: MultiStageResourceCounters,
    pub(crate) push_constant_buffer_index: MultiStageData<Option<ResourceIndex>>,
}

impl PipelineLayout {
    /// Get the first vertex buffer index to be used by attributes.
    #[inline(always)]
    pub(crate) fn attribute_buffer_index(&self) -> ResourceIndex {
        self.total.vs.buffers as _
    }
}

#[derive(Clone)]
pub struct ModuleInfo {
    pub library: metal::Library,
    pub entry_point_map: EntryPointMap,
}

pub struct PipelineCache {
    pub(crate) modules: FastStorageMap<msl::CompilerOptions, FastStorageMap<Vec<u8>, ModuleInfo>>,
}

impl fmt::Debug for PipelineCache {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "PipelineCache")
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct RasterizerState {
    //TODO: more states
    pub front_winding: metal::MTLWinding,
    pub cull_mode: metal::MTLCullMode,
    pub depth_clip: metal::MTLDepthClipMode,
}

impl Default for RasterizerState {
    fn default() -> Self {
        RasterizerState {
            front_winding: metal::MTLWinding::Clockwise,
            cull_mode: metal::MTLCullMode::None,
            depth_clip: metal::MTLDepthClipMode::Clip,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StencilState<T> {
    pub front_reference: T,
    pub back_reference: T,
    pub front_read_mask: T,
    pub back_read_mask: T,
    pub front_write_mask: T,
    pub back_write_mask: T,
}

pub type VertexBufferVec = Vec<(pso::VertexBufferDesc, pso::ElemOffset)>;

#[derive(Debug)]
pub struct GraphicsPipeline {
    // we hold the compiled libraries here for now
    // TODO: move to some cache in `Device`
    pub(crate) vs_lib: metal::Library,
    pub(crate) fs_lib: Option<metal::Library>,
    pub(crate) raw: metal::RenderPipelineState,
    pub(crate) primitive_type: metal::MTLPrimitiveType,
    pub(crate) attribute_buffer_index: ResourceIndex,
    pub(crate) vs_pc_buffer_index: Option<ResourceIndex>,
    pub(crate) ps_pc_buffer_index: Option<ResourceIndex>,
    pub(crate) rasterizer_state: Option<RasterizerState>,
    pub(crate) depth_bias: pso::State<pso::DepthBias>,
    pub(crate) depth_stencil_desc: pso::DepthStencilDesc,
    pub(crate) baked_states: pso::BakedStates,
    /// The mapping from Metal vertex buffers to Vulkan ones.
    /// This is needed because Vulkan allows attribute offsets to exceed the strides,
    /// while Metal does not. Thus, we register extra vertex buffer bindings with
    /// adjusted offsets to cover this use case.
    pub(crate) vertex_buffers: VertexBufferVec,
    /// Tracked attachment formats for figuring (roughly) renderpass compatibility.
    pub(crate) attachment_formats: Vec<Option<Format>>,
}

unsafe impl Send for GraphicsPipeline {}
unsafe impl Sync for GraphicsPipeline {}

#[derive(Debug)]
pub struct ComputePipeline {
    pub(crate) cs_lib: metal::Library,
    pub(crate) raw: metal::ComputePipelineState,
    pub(crate) work_group_size: metal::MTLSize,
    pub(crate) pc_buffer_index: Option<ResourceIndex>,
}

unsafe impl Send for ComputePipeline {}
unsafe impl Sync for ComputePipeline {}

#[derive(Debug)]
pub struct Image {
    pub(crate) raw: metal::Texture,
    pub(crate) kind: image::Kind,
    pub(crate) format_desc: FormatDesc,
    pub(crate) shader_channel: Channel,
    pub(crate) mtl_format: metal::MTLPixelFormat,
    pub(crate) mtl_type: metal::MTLTextureType,
}

impl Image {
    pub(crate) fn pitches_impl(
        extent: image::Extent, format_desc: FormatDesc
    ) -> [buffer::Offset; 3] {
        let bytes_per_texel = format_desc.bits as image::Size >> 3;
        let row_pitch = extent.width * bytes_per_texel;
        let depth_pitch = extent.height * row_pitch;
        let array_pitch = extent.depth * depth_pitch;
        [row_pitch as _, depth_pitch as _, array_pitch as _]
    }
    pub(crate) fn pitches(&self, level: image::Level) -> [buffer::Offset; 3] {
        let extent = self.kind.extent().at_level(level);
        Self::pitches_impl(extent, self.format_desc)
    }
}

unsafe impl Send for Image {}
unsafe impl Sync for Image {}

#[derive(Debug)]
pub struct BufferView {
    pub(crate) raw: metal::Texture,
}

unsafe impl Send for BufferView {}
unsafe impl Sync for BufferView {}

#[derive(Debug)]
pub struct ImageView {
    pub(crate) raw: metal::Texture,
    pub(crate) mtl_format: metal::MTLPixelFormat,
}

unsafe impl Send for ImageView {}
unsafe impl Sync for ImageView {}

#[derive(Debug)]
pub struct Sampler(pub(crate) metal::SamplerState);

unsafe impl Send for Sampler {}
unsafe impl Sync for Sampler {}

#[derive(Debug)]
pub struct Semaphore {
    pub(crate) system: Option<SystemSemaphore>,
    pub(crate) image_ready: Arc<Mutex<Option<SwapchainImage>>>,
}

#[derive(Debug)]
pub struct Buffer {
    pub(crate) raw: metal::Buffer,
    pub(crate) range: Range<u64>,
    pub(crate) res_options: metal::MTLResourceOptions,
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}


#[derive(Debug)]
pub enum DescriptorPool {
    Emulated {
        inner: Arc<RwLock<DescriptorPoolInner>>,
        allocators: ResourceData<RangeAllocator<PoolResourceIndex>>,
    },
    ArgumentBuffer {
        raw: metal::Buffer,
        range_allocator: RangeAllocator<NSUInteger>,
    },
}
//TODO: re-evaluate Send/Sync here
unsafe impl Send for DescriptorPool {}
unsafe impl Sync for DescriptorPool {}

#[derive(Debug)]
pub struct DescriptorPoolInner {
    pub samplers: Vec<Option<SamplerPtr>>,
    pub textures: Vec<Option<(TexturePtr, image::Layout)>>,
    pub buffers: Vec<Option<(BufferPtr, buffer::Offset)>>,
}

impl DescriptorPool {
    pub(crate) fn new_emulated(counters: ResourceData<PoolResourceIndex>) -> Self {
        let inner = DescriptorPoolInner {
            samplers: vec![None; counters.samplers as usize],
            textures: vec![None; counters.textures as usize],
            buffers: vec![None; counters.buffers as usize],
        };
        DescriptorPool::Emulated {
            inner: Arc::new(RwLock::new(inner)),
            allocators: ResourceData {
                samplers: RangeAllocator::new(0 .. counters.samplers),
                textures: RangeAllocator::new(0 .. counters.textures),
                buffers: RangeAllocator::new(0 .. counters.buffers),
            }
        }
    }

    fn report_available(&self) {
        match *self {
            DescriptorPool::Emulated { ref allocators, .. } => {
                trace!("\tavailable {} samplers, {} textures, and {} buffers",
                    allocators.samplers.total_available(),
                    allocators.textures.total_available(),
                    allocators.buffers.total_available(),
                );
            }
            DescriptorPool::ArgumentBuffer { .. } => {}
        }
    }
}

impl HalDescriptorPool<Backend> for DescriptorPool {
    fn allocate_set(&mut self, set_layout: &DescriptorSetLayout) -> Result<DescriptorSet, pso::AllocationError> {
        self.report_available();
        match *self {
            DescriptorPool::Emulated { ref inner, ref mut allocators } => {
                debug!("pool: allocate_set");
                let (layouts, immutable_samplers) = match set_layout {
                    &DescriptorSetLayout::Emulated(ref layouts, ref samplers) => (layouts, samplers),
                    _ => return Err(pso::AllocationError::IncompatibleLayout),
                };

                // step[1]: count the total number of descriptors needed
                let mut counters = MultiStageResourceCounters {
                    vs: ResourceData::new(),
                    ps: ResourceData::new(),
                    cs: ResourceData::new(),
                };
                for layout in layouts.iter() {
                    counters.add(layout.stages, layout.content);
                }
                debug!("\ttotal {:?}", counters);
                let total = ResourceData {
                    buffers: counters.vs.buffers + counters.ps.buffers + counters.cs.buffers,
                    textures: counters.vs.textures + counters.ps.textures + counters.cs.textures,
                    samplers: counters.vs.samplers + counters.ps.samplers + counters.cs.samplers,
                };

                // step[2]: try to allocate the ranges from the pool
                let sampler_range = if total.samplers != 0 {
                    match allocators.samplers.allocate_range(total.samplers as _) {
                        Ok(range) => range,
                        Err(e) => {
                            return Err(if e.fragmented_free_length >= total.samplers {
                                pso::AllocationError::FragmentedPool
                            } else {
                                pso::AllocationError::OutOfPoolMemory
                            });
                        }
                    }
                } else {
                    0 .. 0
                };
                let texture_range = if total.textures != 0 {
                    match allocators.textures.allocate_range(total.textures as _) {
                        Ok(range) => range,
                        Err(e) => {
                            if sampler_range.end != 0 {
                                allocators.samplers.free_range(sampler_range);
                            }
                            return Err(if e.fragmented_free_length >= total.samplers {
                                pso::AllocationError::FragmentedPool
                            } else {
                                pso::AllocationError::OutOfPoolMemory
                            });
                        }
                    }
                } else {
                    0 .. 0
                };
                let buffer_range = if total.buffers != 0 {
                    match allocators.buffers.allocate_range(total.buffers as _) {
                        Ok(range) => range,
                        Err(e) => {
                            if sampler_range.end != 0 {
                                allocators.samplers.free_range(sampler_range);
                            }
                            if texture_range.end != 0 {
                                allocators.textures.free_range(texture_range);
                            }
                            return Err(if e.fragmented_free_length >= total.samplers {
                                pso::AllocationError::FragmentedPool
                            } else {
                                pso::AllocationError::OutOfPoolMemory
                            });
                        }
                    }
                } else {
                    0 .. 0
                };

                // step[3]: fill out immutable samplers
                if !immutable_samplers.is_empty() {
                    let mut data = inner.write();
                    let mut data_vs_index = sampler_range.start as usize;
                    let mut data_ps_index = data_vs_index + counters.vs.samplers as usize;
                    let mut data_cs_index = data_ps_index + counters.ps.samplers as usize;
                    let mut sampler_iter = immutable_samplers.iter();

                    for layout in layouts.iter() {
                        if layout.content.contains(DescriptorContent::SAMPLER) {
                            let value = if layout.content.contains(DescriptorContent::IMMUTABLE_SAMPLER) {
                                Some(AsNative::from(sampler_iter.next().unwrap().as_ref()))
                            } else {
                                None
                            };
                            if layout.stages.contains(pso::ShaderStageFlags::VERTEX) {
                                data.samplers[data_vs_index] = value;
                                data_vs_index += 1;
                            }
                            if layout.stages.contains(pso::ShaderStageFlags::FRAGMENT) {
                                data.samplers[data_ps_index] = value;
                                data_ps_index += 1;
                            }
                            if layout.stages.contains(pso::ShaderStageFlags::COMPUTE) {
                                data.samplers[data_cs_index] = value;
                                data_cs_index += 1;
                            }
                        }
                    }
                    debug!("\tassigning {} immutable_samplers", immutable_samplers.len());
                }

                let resources = {
                    let vs = ResourceData {
                        buffers: buffer_range.start .. buffer_range.start + counters.vs.buffers,
                        textures: texture_range.start .. texture_range.start + counters.vs.textures,
                        samplers: sampler_range.start .. sampler_range.start + counters.vs.samplers,
                    };
                    let ps = ResourceData {
                        buffers: vs.buffers.end .. vs.buffers.end + counters.ps.buffers,
                        textures: vs.textures.end .. vs.textures.end + counters.ps.textures,
                        samplers: vs.samplers.end .. vs.samplers.end + counters.ps.samplers,
                    };
                    let cs = ResourceData {
                        buffers: ps.buffers.end .. buffer_range.end,
                        textures: ps.textures.end .. texture_range.end,
                        samplers: ps.samplers.end .. sampler_range.end,
                    };
                    MultiStageData { vs, ps, cs }
                };

                Ok(DescriptorSet::Emulated {
                    pool: Arc::clone(inner),
                    layouts: Arc::clone(layouts),
                    resources,
                })
            }
            DescriptorPool::ArgumentBuffer { ref raw, ref mut range_allocator, } => {
                let (encoder, stage_flags) = match set_layout {
                    &DescriptorSetLayout::ArgumentBuffer(ref encoder, stages) => (encoder, stages),
                    _ => return Err(pso::AllocationError::IncompatibleLayout),
                };
                match range_allocator.allocate_range(encoder.encoded_length()) {
                    Ok(range) => Ok(DescriptorSet::ArgumentBuffer {
                        raw: raw.clone(),
                        offset: range.start,
                        encoder: encoder.clone(),
                        stage_flags,
                    }),
                    Err(_) => Err(pso::AllocationError::OutOfPoolMemory),
                }
            }
        }
    }

    fn free_sets<I>(&mut self, descriptor_sets: I)
    where
        I: IntoIterator<Item = DescriptorSet>
    {
        match self {
            DescriptorPool::Emulated { ref inner, ref mut allocators } => {
                debug!("pool: free_sets");
                let mut data = inner.write();
                for descriptor_set in descriptor_sets {
                    match descriptor_set {
                        DescriptorSet::Emulated { resources, .. } => {
                            debug!("\t{:?} resources", resources);
                            let sampler_range = resources.vs.samplers.start .. resources.cs.samplers.end;
                            for sampler in &mut data.samplers[sampler_range.start as usize .. sampler_range.end as usize] {
                                *sampler = None;
                            }
                            if sampler_range.start != sampler_range.end {
                                allocators.samplers.free_range(sampler_range);
                            }
                            let texture_range = resources.vs.textures.start .. resources.cs.textures.end;
                            for image in &mut data.textures[texture_range.start as usize .. texture_range.end as usize] {
                                *image = None;
                            }
                            if texture_range.start != texture_range.end {
                                allocators.textures.free_range(texture_range);
                            }
                            let buffer_range = resources.vs.buffers.start .. resources.cs.buffers.end;
                            for buffer in &mut data.buffers[buffer_range.start as usize .. buffer_range.end as usize] {
                                *buffer = None;
                            }
                            if buffer_range.start != buffer_range.end {
                                allocators.buffers.free_range(buffer_range);
                            }
                        }
                        DescriptorSet::ArgumentBuffer{..} => {
                            panic!("Tried to free a DescriptorSet not given out by this DescriptorPool!")
                        }
                    }
                }
            }
            DescriptorPool::ArgumentBuffer { ref mut range_allocator, .. } => {
                for descriptor_set in descriptor_sets {
                    match descriptor_set {
                        DescriptorSet::Emulated{..} => {
                            panic!("Tried to free a DescriptorSet not given out by this DescriptorPool!")
                        }
                        DescriptorSet::ArgumentBuffer { offset, encoder, .. } => {
                            let handle_range = offset .. offset + encoder.encoded_length();
                            range_allocator.free_range(handle_range);
                        }
                    }
                }
            }
        }
        self.report_available();
    }

    fn reset(&mut self) {
        match *self {
            DescriptorPool::Emulated { ref inner, ref mut allocators } => {
                debug!("pool: reset");
                if allocators.samplers.is_empty() && allocators.textures.is_empty() && allocators.buffers.is_empty() {
                    return // spare the locking
                }
                let mut data = inner.write();

                for range in allocators.samplers.allocated_ranges() {
                    for sampler in &mut data.samplers[range.start as usize .. range.end as usize] {
                        *sampler = None;
                    }
                }
                for range in allocators.textures.allocated_ranges() {
                    for texture in &mut data.textures[range.start as usize .. range.end as usize] {
                        *texture = None;
                    }
                }
                for range in allocators.buffers.allocated_ranges() {
                    for buffer in &mut data.buffers[range.start as usize .. range.end as usize] {
                        *buffer = None;
                    }
                }

                allocators.samplers.reset();
                allocators.textures.reset();
                allocators.buffers.reset();
            }
            DescriptorPool::ArgumentBuffer { ref mut range_allocator, .. } => {
                range_allocator.reset();
            }
        }
    }
}

bitflags! {
    /// Descriptor content flags.
    pub struct DescriptorContent: u8 {
        const BUFFER = 1<<0;
        const DYNAMIC_BUFFER = 1<<1;
        const TEXTURE = 1<<2;
        const SAMPLER = 1<<3;
        const IMMUTABLE_SAMPLER = 1<<4;
    }
}

impl From<pso::DescriptorType> for DescriptorContent {
    fn from(ty: pso::DescriptorType) -> Self {
        match ty {
            pso::DescriptorType::Sampler => {
                DescriptorContent::SAMPLER
            }
            pso::DescriptorType::CombinedImageSampler => {
                DescriptorContent::TEXTURE | DescriptorContent::SAMPLER
            }
            pso::DescriptorType::SampledImage |
            pso::DescriptorType::StorageImage |
            pso::DescriptorType::UniformTexelBuffer |
            pso::DescriptorType::StorageTexelBuffer |
            pso::DescriptorType::InputAttachment => {
                DescriptorContent::TEXTURE
            }
            pso::DescriptorType::UniformBuffer |
            pso::DescriptorType::StorageBuffer => {
                DescriptorContent::BUFFER
            }
            pso::DescriptorType::UniformBufferDynamic |
            pso::DescriptorType::StorageBufferDynamic => {
                DescriptorContent::BUFFER | DescriptorContent::DYNAMIC_BUFFER
            }
        }
    }
}

// Note: this structure is iterated often, so it makes sense to keep it dense
#[derive(Debug)]
pub struct DescriptorLayout {
    pub content: DescriptorContent,
    pub stages: pso::ShaderStageFlags,
    pub binding: pso::DescriptorBinding,
    pub array_index: pso::DescriptorArrayIndex,
}

#[derive(Debug)]
pub enum DescriptorSetLayout {
    Emulated(Arc<Vec<DescriptorLayout>>, Vec<metal::SamplerState>),
    ArgumentBuffer(metal::ArgumentEncoder, pso::ShaderStageFlags),
}
unsafe impl Send for DescriptorSetLayout {}
unsafe impl Sync for DescriptorSetLayout {}

#[derive(Debug)]
pub enum DescriptorSet {
    Emulated {
        pool: Arc<RwLock<DescriptorPoolInner>>,
        layouts: Arc<Vec<DescriptorLayout>>,
        resources: MultiStageData<ResourceData<Range<PoolResourceIndex>>>,
    },
    ArgumentBuffer {
        raw: metal::Buffer,
        offset: NSUInteger,
        encoder: metal::ArgumentEncoder,
        stage_flags: pso::ShaderStageFlags,
    },
}
unsafe impl Send for DescriptorSet {}
unsafe impl Sync for DescriptorSet {}


#[derive(Debug)]
pub struct Memory {
    pub(crate) heap: MemoryHeap,
    pub(crate) size: u64,
}

impl Memory {
    pub(crate) fn new(heap: MemoryHeap, size: u64) -> Self {
        Memory {
            heap,
            size,
        }
    }

    pub(crate) fn resolve<R: RangeArg<u64>>(&self, range: &R) -> Range<u64> {
        *range.start().unwrap_or(&0) .. *range.end().unwrap_or(&self.size)
    }
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}

#[derive(Debug)]
pub(crate) enum MemoryHeap {
    Private,
    Public(MemoryTypeId, metal::Buffer),
    Native(metal::Heap),
}

#[derive(Debug)]
pub struct UnboundBuffer {
    pub(crate) size: u64,
    pub(crate) usage: buffer::Usage,
}
unsafe impl Send for UnboundBuffer {}
unsafe impl Sync for UnboundBuffer {}

#[derive(Debug)]
pub struct UnboundImage {
    pub(crate) texture_desc: metal::TextureDescriptor,
    pub(crate) format: Format,
    pub(crate) kind: image::Kind,
    pub(crate) mip_sizes: Vec<u64>,
    pub(crate) host_visible: bool,
}
unsafe impl Send for UnboundImage {}
unsafe impl Sync for UnboundImage {}

#[derive(Debug)]
pub enum QueryPool {
    Occlusion(Range<u32>),
}

#[derive(Debug)]
pub enum FenceInner {
    Idle { signaled: bool },
    Pending(metal::CommandBuffer),
}

#[derive(Debug)]
pub struct Fence(pub(crate) RefCell<FenceInner>);

unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}


extern "C" {
    fn dispatch_semaphore_wait(
        semaphore: *mut c_void,
        timeout: u64,
    ) -> c_long;

    fn dispatch_semaphore_signal(
        semaphore: *mut c_void,
    ) -> c_long;

    fn dispatch_semaphore_create(
        value: c_long,
    ) -> *mut c_void;

    fn dispatch_release(
        object: *mut c_void,
    );
}

#[derive(Clone, Debug)]
pub struct SystemSemaphore(*mut c_void);
unsafe impl Send for SystemSemaphore {}
unsafe impl Sync for SystemSemaphore {}

impl Drop for SystemSemaphore {
    fn drop(&mut self) {
        unsafe {
            dispatch_release(self.0)
        }
    }
}
impl SystemSemaphore {
    pub(crate) fn new() -> Self {
        SystemSemaphore(unsafe {
            dispatch_semaphore_create(1)
        })
    }
    pub(crate) fn signal(&self) {
        unsafe {
            dispatch_semaphore_signal(self.0);
        }
    }
    pub(crate) fn wait(&self, timeout: u64) {
        unsafe {
            dispatch_semaphore_wait(self.0, timeout);
        }
    }
}
