use {AsNative, Backend, ResourceIndex, BufferPtr, SamplerPtr, TexturePtr};
use internal::{Channel, FastStorageMap};
use range_alloc::RangeAllocator;
use window::SwapchainImage;

use std::cell::RefCell;
use std::fmt;
use std::ops::Range;
use std::os::raw::{c_void, c_long};
use std::sync::Arc;

use hal::{buffer, image, pso};
use hal::{DescriptorPool as HalDescriptorPool, MemoryTypeId};
use hal::backend::FastHashMap;
use hal::format::{Format, FormatDesc};
use hal::pass::{Attachment, AttachmentId};
use hal::range::RangeArg;

use cocoa::foundation::{NSRange, NSUInteger};
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


bitflags! {
    /// Subpass attachment operations.
    pub struct SubpassOps: u8 {
        const LOAD = 0x0;
        const STORE = 0x1;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SubpassFormats {
    pub colors: SmallVec<[(metal::MTLPixelFormat, Channel); 4]>,
    pub depth_stencil: Option<metal::MTLPixelFormat>,
}

impl SubpassFormats {
    pub fn copy_from(&mut self, other: &Self) {
        self.colors.clear();
        self.colors.extend_from_slice(&other.colors);
        self.depth_stencil = other.depth_stencil;
    }
}

#[derive(Debug)]
pub struct Subpass {
    pub colors: Vec<(AttachmentId, SubpassOps)>,
    pub depth_stencil: Option<(AttachmentId, SubpassOps)>,
    pub inputs: Vec<AttachmentId>,
    pub target_formats: SubpassFormats,
}

#[derive(Debug)]
pub struct RenderPass {
    pub(crate) attachments: Vec<Attachment>,
    pub(crate) subpasses: Vec<Subpass>,
}

unsafe impl Send for RenderPass {}
unsafe impl Sync for RenderPass {}


#[derive(Debug)]
pub struct Framebuffer {
    pub(crate) extent: image::Extent,
    pub(crate) attachments: Vec<metal::Texture>,
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

impl<T: Copy + Ord> ResourceData<Range<T>> {
    pub fn expand(&mut self, point: ResourceData<T>) {
        //TODO: modify `start` as well?
        self.buffers.end = self.buffers.end.max(point.buffers);
        self.textures.end = self.textures.end.max(point.textures);
        self.samplers.end = self.samplers.end.max(point.samplers);
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

#[derive(Debug)]
pub struct DescriptorSetInfo {
    pub offsets: MultiStageResourceCounters,
    pub dynamic_buffers: Vec<MultiStageData<PoolResourceIndex>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PushConstantInfo {
    pub count: u32,
    pub buffer_index: ResourceIndex,
}

#[derive(Debug)]
pub struct PipelineLayout {
    pub(crate) shader_compiler_options: msl::CompilerOptions,
    pub(crate) shader_compiler_options_point: msl::CompilerOptions,
    pub(crate) infos: Vec<DescriptorSetInfo>,
    pub(crate) total: MultiStageResourceCounters,
    pub(crate) push_constants: MultiStageData<Option<PushConstantInfo>>,
    pub(crate) total_push_constants: u32,
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
    pub rasterization_enabled: bool,
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
    pub(crate) vs_pc_info: Option<PushConstantInfo>,
    pub(crate) ps_pc_info: Option<PushConstantInfo>,
    pub(crate) rasterizer_state: Option<RasterizerState>,
    pub(crate) depth_bias: pso::State<pso::DepthBias>,
    pub(crate) depth_stencil_desc: pso::DepthStencilDesc,
    pub(crate) baked_states: pso::BakedStates,
    /// The mapping from Metal vertex buffers to Vulkan ones.
    /// This is needed because Vulkan allows attribute offsets to exceed the strides,
    /// while Metal does not. Thus, we register extra vertex buffer bindings with
    /// adjusted offsets to cover this use case.
    pub(crate) vertex_buffers: VertexBufferVec,
    /// Tracked attachment formats
    pub(crate) attachment_formats: SubpassFormats,
}

unsafe impl Send for GraphicsPipeline {}
unsafe impl Sync for GraphicsPipeline {}

#[derive(Debug)]
pub struct ComputePipeline {
    pub(crate) cs_lib: metal::Library,
    pub(crate) raw: metal::ComputePipelineState,
    pub(crate) work_group_size: metal::MTLSize,
    pub(crate) pc_info: Option<PushConstantInfo>,
}

unsafe impl Send for ComputePipeline {}
unsafe impl Sync for ComputePipeline {}

#[derive(Debug)]
pub enum ImageLike {
    /// This is a linearly tiled HOST-visible image, which is represented by a buffer.
    Buffer(Buffer),
    /// This is a regular image represented by a texture.
    Texture(metal::Texture),
}

impl ImageLike {
    pub fn as_texture(&self) -> &metal::TextureRef {
        match *self {
            ImageLike::Buffer(..) => panic!("Unexpected buffer-backed image"),
            ImageLike::Texture(ref tex) => tex,
        }
    }
}

#[derive(Debug)]
pub struct Image {
    pub(crate) like: ImageLike,
    pub(crate) kind: image::Kind,
    pub(crate) format_desc: FormatDesc,
    pub(crate) shader_channel: Channel,
    pub(crate) mtl_format: metal::MTLPixelFormat,
    pub(crate) mtl_type: metal::MTLTextureType,
}

impl Image {
    pub(crate) fn pitches_impl(
        extent: image::Extent, format_desc: FormatDesc
    ) -> [buffer::Offset; 4] {
        let bytes_per_texel = format_desc.bits as image::Size >> 3;
        let row_pitch = extent.width * bytes_per_texel;
        let depth_pitch = extent.height * row_pitch;
        let array_pitch = extent.depth * depth_pitch;
        [bytes_per_texel as _, row_pitch as _, depth_pitch as _, array_pitch as _]
    }
    pub(crate) fn pitches(&self, level: image::Level) -> [buffer::Offset; 4] {
        let extent = self.kind.extent().at_level(level);
        Self::pitches_impl(extent, self.format_desc)
    }
    pub(crate) fn byte_offset(&self, offset: image::Offset) -> buffer::Offset {
        let pitches = Self::pitches_impl(self.kind.extent(), self.format_desc);
        pitches[0] * offset.x as buffer::Offset +
        pitches[1] * offset.y as buffer::Offset +
        pitches[2] * offset.z as buffer::Offset
    }
    pub(crate) fn byte_extent(&self, extent: image::Extent) -> buffer::Offset {
        let bytes_per_texel = self.format_desc.bits as image::Size >> 3;
        (bytes_per_texel * extent.width * extent.height * extent.depth) as _
    }
    /// View this cube texture as a 2D array.
    pub(crate) fn view_cube_as_2d(&self) -> Option<metal::Texture> {
        match self.mtl_type {
            metal::MTLTextureType::Cube |
            metal::MTLTextureType::CubeArray => {
                let raw = self.like.as_texture();
                Some(raw.new_texture_view_from_slice(
                    self.mtl_format,
                    metal::MTLTextureType::D2Array,
                    NSRange {
                        location: 0,
                        length: raw.mipmap_level_count(),
                    },
                    NSRange {
                        location: 0,
                        length: self.kind.num_layers() as _,
                    },
                ))
            }
            _ => None,
        }
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
    pub(crate) options: metal::MTLResourceOptions,
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
                let mut total = ResourceData::new();
                for layout in layouts.iter() {
                    total.add(layout.content);
                }
                debug!("\ttotal {:?}", total);

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
                    let mut data_iter = data.samplers[sampler_range.start as usize .. sampler_range.end as usize].iter_mut();
                    let mut sampler_iter = immutable_samplers.iter();

                    for layout in layouts.iter() {
                        if layout.content.contains(DescriptorContent::SAMPLER) {
                            *data_iter.next().unwrap() = if layout.content.contains(DescriptorContent::IMMUTABLE_SAMPLER) {
                                Some(AsNative::from(sampler_iter.next().unwrap().as_ref()))
                            } else {
                                None
                            };
                        }
                    }
                    debug!("\tassigning {} immutable_samplers", immutable_samplers.len());
                }

                let resources = ResourceData {
                    buffers: buffer_range,
                    textures: texture_range,
                    samplers: sampler_range,
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
                            for sampler in &mut data.samplers[resources.samplers.start as usize .. resources.samplers.end as usize] {
                                *sampler = None;
                            }
                            if resources.samplers.start != resources.samplers.end {
                                allocators.samplers.free_range(resources.samplers);
                            }
                            for image in &mut data.textures[resources.textures.start as usize .. resources.textures.end as usize] {
                                *image = None;
                            }
                            if resources.textures.start != resources.textures.end {
                                allocators.textures.free_range(resources.textures);
                            }
                            for buffer in &mut data.buffers[resources.buffers.start as usize .. resources.buffers.end as usize] {
                                *buffer = None;
                            }
                            if resources.buffers.start != resources.buffers.end {
                                allocators.buffers.free_range(resources.buffers);
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
        resources: ResourceData<Range<PoolResourceIndex>>,
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
