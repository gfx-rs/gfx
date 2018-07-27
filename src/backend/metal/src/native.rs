use {Backend, BufferPtr, SamplerPtr, TexturePtr};
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
use foreign_types::ForeignType;
use metal;
use parking_lot::{Mutex, RwLock};
use smallvec::SmallVec;
use spirv_cross::{msl, spirv};


pub type EntryPointMap = FastHashMap<String, spirv::EntryPoint>;

/// Shader module can be compiled in advance if it's resource bindings do not
/// depend on pipeline layout, in which case the value would become `Compiled`.
pub enum ShaderModule {
    Compiled {
        library: metal::Library,
        entry_point_map: EntryPointMap,
    },
    Raw(Vec<u8>),
}

impl fmt::Debug for ShaderModule {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ShaderModule::Compiled { .. } => {
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

pub type ResourceOverrideMap = FastHashMap<msl::ResourceBindingLocation, msl::ResourceBinding>;

#[derive(Clone, Debug)]
pub struct ResourceCounters {
    pub buffers: usize,
    pub textures: usize,
    pub samplers: usize,
}

impl ResourceCounters {
    pub fn new() -> Self {
        ResourceCounters {
            buffers: 0,
            textures: 0,
            samplers: 0,
        }
    }

    pub fn add(&mut self, content: DescriptorContent) {
        if content.contains(DescriptorContent::BUFFER) {
            self.buffers += 1;
        }
        if content.contains(DescriptorContent::TEXTURE) {
            self.textures += 1;
        }
        if content.contains(DescriptorContent::SAMPLER) {
            self.samplers += 1;
        }
    }
}

#[derive(Clone, Debug)]
pub struct MultiStageResourceCounters {
    pub vs: ResourceCounters,
    pub ps: ResourceCounters,
    pub cs: ResourceCounters,
}

#[derive(Debug)]
pub struct PipelineLayout {
    pub(crate) res_overrides: ResourceOverrideMap,
    pub(crate) offsets: Vec<MultiStageResourceCounters>,
    pub(crate) total: MultiStageResourceCounters,
}

impl PipelineLayout {
    /// Get the first vertex buffer index to be used by attributes.
    #[inline(always)]
    pub(crate) fn attribute_buffer_index(&self) -> u32 {
        self.total.vs.buffers as _
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

pub type VertexBufferMap = FastHashMap<(pso::BufferIndex, pso::ElemOffset), pso::VertexBufferDesc>;

#[derive(Debug)]
pub struct GraphicsPipeline {
    // we hold the compiled libraries here for now
    // TODO: move to some cache in `Device`
    pub(crate) vs_lib: metal::Library,
    pub(crate) fs_lib: Option<metal::Library>,
    pub(crate) raw: metal::RenderPipelineState,
    pub(crate) primitive_type: metal::MTLPrimitiveType,
    pub(crate) attribute_buffer_index: u32,
    pub(crate) rasterizer_state: Option<RasterizerState>,
    pub(crate) depth_bias: pso::State<pso::DepthBias>,
    pub(crate) depth_stencil_desc: pso::DepthStencilDesc,
    pub(crate) baked_states: pso::BakedStates,
    /// The mapping of additional vertex buffer bindings over the original ones.
    /// This is needed because Vulkan allows attribute offsets to exceed the strides,
    /// while Metal does not. Thus, we register extra vertex buffer bindings with
    /// adjusted offsets to cover this use case.
    pub(crate) vertex_buffer_map: VertexBufferMap,
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
        sampler_alloc: RangeAllocator<pso::DescriptorBinding>,
        texture_alloc: RangeAllocator<pso::DescriptorBinding>,
        buffer_alloc: RangeAllocator<pso::DescriptorBinding>,
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
    pub(crate) fn new_emulated(num_samplers: usize, num_textures: usize, num_buffers: usize) -> Self {
        let inner = DescriptorPoolInner {
            samplers: vec![None; num_samplers],
            textures: vec![None; num_textures],
            buffers: vec![None; num_buffers],
        };
        DescriptorPool::Emulated {
            inner: Arc::new(RwLock::new(inner)),
            sampler_alloc: RangeAllocator::new(0 .. num_samplers as pso::DescriptorBinding),
            texture_alloc: RangeAllocator::new(0 .. num_textures as pso::DescriptorBinding),
            buffer_alloc: RangeAllocator::new(0 .. num_buffers as pso::DescriptorBinding),
        }
    }

    fn report_available(&self) {
        match *self {
            DescriptorPool::Emulated { ref sampler_alloc, ref texture_alloc, ref buffer_alloc, .. } => {
                trace!("\tavailable {} samplers, {} textures, and {} buffers",
                    sampler_alloc.total_available(),
                    texture_alloc.total_available(),
                    buffer_alloc.total_available(),
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
            DescriptorPool::Emulated { ref inner, ref mut sampler_alloc, ref mut texture_alloc, ref mut buffer_alloc } => {
                debug!("pool: allocate_set");
                let (layouts, immutable_samplers) = match set_layout {
                    &DescriptorSetLayout::Emulated(ref layouts, ref samplers) => (layouts, samplers),
                    _ => return Err(pso::AllocationError::IncompatibleLayout),
                };

                // step[1]: count the total number of descriptors needed
                let mut total = ResourceCounters::new();
                let mut has_immutable_samplers = false;
                for layout in layouts.iter() {
                    total.add(layout.content);
                    has_immutable_samplers |= layout.content.contains(DescriptorContent::IMMUTABLE_SAMPLER);
                }
                debug!("\ttotal {:?}", total);

                // step[2]: try to allocate the ranges from the pool
                let sampler_range = if total.samplers != 0 {
                    match sampler_alloc.allocate_range(total.samplers as _) {
                        Ok(range) => range,
                        Err(e) => {
                            return Err(if e.fragmented_free_length >= total.samplers as u32 {
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
                    match texture_alloc.allocate_range(total.textures as _) {
                        Ok(range) => range,
                        Err(e) => {
                            if sampler_range.end != 0 {
                                sampler_alloc.free_range(sampler_range);
                            }
                            return Err(if e.fragmented_free_length >= total.samplers as u32 {
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
                    match buffer_alloc.allocate_range(total.buffers as _) {
                        Ok(range) => range,
                        Err(e) => {
                            if sampler_range.end != 0 {
                                sampler_alloc.free_range(sampler_range);
                            }
                            if texture_range.end != 0 {
                                texture_alloc.free_range(texture_range);
                            }
                            return Err(if e.fragmented_free_length >= total.samplers as u32 {
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
                if has_immutable_samplers {
                    let mut data = inner.write();
                    let mut sampler_offset = sampler_range.start as usize;

                    for layout in layouts.iter() {
                        if layout.content.contains(DescriptorContent::SAMPLER) {
                            if layout.content.contains(DescriptorContent::IMMUTABLE_SAMPLER) {
                                let value = &immutable_samplers[layout.associated_data_index as usize];
                                data.samplers[sampler_offset] = Some(SamplerPtr(value.as_ptr()));
                            }
                            sampler_offset += 1;
                        }
                    }
                    debug!("\tassigning {} immutable_samplers", immutable_samplers.len());
                }

                Ok(DescriptorSet::Emulated {
                    pool: Arc::clone(inner),
                    layouts: Arc::clone(layouts),
                    sampler_range,
                    texture_range,
                    buffer_range,
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
            DescriptorPool::Emulated { ref inner, ref mut sampler_alloc, ref mut texture_alloc, ref mut buffer_alloc } => {
                debug!("pool: free_sets");
                let mut data = inner.write();
                for descriptor_set in descriptor_sets {
                    match descriptor_set {
                        DescriptorSet::Emulated { sampler_range, texture_range, buffer_range, .. } => {
                            debug!("\t{:?} samplers, {:?} textures, and {:?} buffers",
                                sampler_range, texture_range, buffer_range);
                            for sampler in &mut data.samplers[sampler_range.start as usize .. sampler_range.end as usize] {
                                *sampler = None;
                            }
                            if sampler_range.start != sampler_range.end {
                                sampler_alloc.free_range(sampler_range);
                            }
                            for image in &mut data.textures[texture_range.start as usize .. texture_range.end as usize] {
                                *image = None;
                            }
                            if texture_range.start != texture_range.end {
                                texture_alloc.free_range(texture_range);
                            }
                            for buffer in &mut data.buffers[buffer_range.start as usize .. buffer_range.end as usize] {
                                *buffer = None;
                            }
                            if buffer_range.start != buffer_range.end {
                                buffer_alloc.free_range(buffer_range);
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
            DescriptorPool::Emulated { ref inner, ref mut sampler_alloc, ref mut texture_alloc, ref mut buffer_alloc } => {
                debug!("pool: reset");
                if sampler_alloc.is_empty() && texture_alloc.is_empty() && buffer_alloc.is_empty() {
                    return // spare the locking
                }
                let mut data = inner.write();

                for range in sampler_alloc.allocated_ranges() {
                    for sampler in &mut data.samplers[range.start as usize .. range.end as usize] {
                        *sampler = None;
                    }
                }
                for range in texture_alloc.allocated_ranges() {
                    for texture in &mut data.textures[range.start as usize .. range.end as usize] {
                        *texture = None;
                    }
                }
                for range in buffer_alloc.allocated_ranges() {
                    for buffer in &mut data.buffers[range.start as usize .. range.end as usize] {
                        *buffer = None;
                    }
                }

                sampler_alloc.reset();
                texture_alloc.reset();
                buffer_alloc.reset();
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
    /// Index of either an immutable sampler or a dynamic offset entry, if applicable
    pub associated_data_index: u16,
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
        sampler_range: Range<pso::DescriptorBinding>,
        texture_range: Range<pso::DescriptorBinding>,
        buffer_range: Range<pso::DescriptorBinding>
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
