use {Backend, BufferPtr, SamplerPtr, TexturePtr};
use internal::Channel;
use range_alloc::RangeAllocator;
use window::SwapchainImage;

use std::cell::RefCell;
use std::fmt;
use std::ops::Range;
use std::os::raw::{c_void, c_long};
use std::sync::Arc;

use hal::{self, image, pso};
use hal::backend::FastHashMap;
use hal::format::{Aspects, Format, FormatDesc};

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

#[derive(Debug)]
pub struct RenderPass {
    pub(crate) attachments: Vec<hal::pass::Attachment>,
}

unsafe impl Send for RenderPass {}
unsafe impl Sync for RenderPass {}

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
    pub(crate) descriptor: Mutex<metal::RenderPassDescriptor>,
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
}

#[derive(Debug)]
pub struct PipelineLayout {
    pub(crate) res_overrides: ResourceOverrideMap,
    pub(crate) vs_counters: ResourceCounters,
    pub(crate) ps_counters: ResourceCounters,
    pub(crate) cs_counters: ResourceCounters,
}

impl PipelineLayout {
    /// Get the first vertex buffer index to be used by attributes.
    #[inline(always)]
    pub(crate) fn attribute_buffer_index(&self) -> u32 {
        self.vs_counters.buffers as _
    }
}

#[derive(Clone, Debug)]
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
    ) -> [hal::buffer::Offset; 3] {
        let bytes_per_texel = format_desc.bits as image::Size >> 3;
        let row_pitch = extent.width * bytes_per_texel;
        let depth_pitch = extent.height * row_pitch;
        let array_pitch = extent.depth * depth_pitch;
        [row_pitch as _, depth_pitch as _, array_pitch as _]
    }
    pub(crate) fn pitches(&self, level: image::Level) -> [hal::buffer::Offset; 3] {
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

#[derive(Clone, Debug)]
pub struct BufferBinding {
    pub base: Option<(BufferPtr, u64)>,
    pub dynamic: bool,
}

#[derive(Debug)]
pub struct DescriptorPoolInner {
    pub samplers: Vec<Option<SamplerPtr>>,
    pub textures: Vec<Option<(TexturePtr, image::Layout)>>,
    pub buffers: Vec<BufferBinding>,
}

impl DescriptorPool {
    pub(crate) fn new_emulated(num_samplers: usize, num_textures: usize, num_buffers: usize) -> Self {
        let inner = DescriptorPoolInner {
            samplers: vec![None; num_samplers],
            textures: vec![None; num_textures],
            buffers: vec![BufferBinding { base: None, dynamic: false }; num_buffers],
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

    pub(crate) fn count_bindings(
        desc_type: pso::DescriptorType,
        desc_count: usize,
        num_samplers: &mut usize,
        num_textures: &mut usize,
        num_buffers: &mut usize,
    ) {
        match desc_type {
            pso::DescriptorType::Sampler => {
                *num_samplers += desc_count;
            }
            pso::DescriptorType::CombinedImageSampler => {
                *num_samplers += desc_count;
                *num_textures += desc_count;
            }
            pso::DescriptorType::SampledImage |
            pso::DescriptorType::StorageImage |
            pso::DescriptorType::UniformTexelBuffer |
            pso::DescriptorType::StorageTexelBuffer |
            pso::DescriptorType::InputAttachment => {
                *num_textures += desc_count;
            }
            pso::DescriptorType::UniformBuffer |
            pso::DescriptorType::StorageBuffer |
            pso::DescriptorType::UniformBufferDynamic |
            pso::DescriptorType::StorageBufferDynamic => {
                *num_buffers += desc_count;
            }
        };
    }
}

impl hal::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_set(&mut self, set_layout: &DescriptorSetLayout) -> Result<DescriptorSet, pso::AllocationError> {
        self.report_available();
        match *self {
            DescriptorPool::Emulated { ref inner, ref mut sampler_alloc, ref mut texture_alloc, ref mut buffer_alloc } => {
                debug!("pool: allocate_set");
                let (layout_bindings, immutable_samplers) = match set_layout {
                    &DescriptorSetLayout::Emulated(ref bindings, ref samplers) => (bindings, samplers),
                    _ => return Err(pso::AllocationError::IncompatibleLayout),
                };

                // step[1]: count the total number of descriptors needed
                let mut total_samplers = 0;
                let mut total_textures = 0;
                let mut total_buffers = 0;
                let mut has_immutable_samplers = false;
                for layout in layout_bindings.iter() {
                    has_immutable_samplers |= layout.immutable_samplers;
                    Self::count_bindings(layout.ty, layout.count,
                        &mut total_samplers, &mut total_textures, &mut total_buffers);
                }
                debug!("\t{} sampler, {} texture, and {} buffer sets",
                    total_samplers, total_textures, total_buffers);

                // step[2]: try to allocate the ranges from the pool
                let sampler_range = if total_samplers != 0 {
                    match sampler_alloc.allocate_range(total_samplers as _) {
                        Ok(range) => range,
                        Err(e) => {
                            return Err(if e.fragmented_free_length >= total_samplers as u32 {
                                pso::AllocationError::FragmentedPool
                            } else {
                                pso::AllocationError::OutOfPoolMemory
                            });
                        }
                    }
                } else {
                    0 .. 0
                };
                let texture_range = if total_textures != 0 {
                    match texture_alloc.allocate_range(total_textures as _) {
                        Ok(range) => range,
                        Err(e) => {
                            if sampler_range.end != 0 {
                                sampler_alloc.free_range(sampler_range);
                            }
                            return Err(if e.fragmented_free_length >= total_samplers as u32 {
                                pso::AllocationError::FragmentedPool
                            } else {
                                pso::AllocationError::OutOfPoolMemory
                            });
                        }
                    }
                } else {
                    0 .. 0
                };
                let buffer_range = if total_buffers != 0 {
                    match buffer_alloc.allocate_range(total_buffers as _) {
                        Ok(range) => range,
                        Err(e) => {
                            if sampler_range.end != 0 {
                                sampler_alloc.free_range(sampler_range);
                            }
                            if texture_range.end != 0 {
                                texture_alloc.free_range(texture_range);
                            }
                            return Err(if e.fragmented_free_length >= total_samplers as u32 {
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
                    let mut immutable_sampler_offset = 0;
                    let mut sampler_offset = sampler_range.start as usize;
                    let (mut tx_temp, mut bf_temp) = (0, 0);

                    for layout in layout_bindings.iter() {
                        if layout.immutable_samplers {
                            for (sampler, immutable) in data
                                .samplers[sampler_offset .. sampler_offset + layout.count]
                                .iter_mut()
                                .zip(&immutable_samplers[immutable_sampler_offset..])
                            {
                                *sampler = Some(SamplerPtr(immutable.as_ptr()))
                            }
                            immutable_sampler_offset += layout.count;
                        }
                        Self::count_bindings(layout.ty, layout.count, &mut sampler_offset, &mut tx_temp, &mut bf_temp);
                    }

                    assert_eq!(immutable_sampler_offset, immutable_samplers.len());
                    debug!("\tassigning {} immutable_samplers", immutable_samplers.len());
                }

                Ok(DescriptorSet::Emulated {
                    pool: Arc::clone(inner),
                    layouts: Arc::clone(layout_bindings),
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
                                buffer.base = None;
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
                        buffer.base = None;
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

#[derive(Debug)]
pub enum DescriptorSetLayout {
    Emulated(Arc<Vec<pso::DescriptorSetLayoutBinding>>, Vec<metal::SamplerState>),
    ArgumentBuffer(metal::ArgumentEncoder, pso::ShaderStageFlags),
}
unsafe impl Send for DescriptorSetLayout {}
unsafe impl Sync for DescriptorSetLayout {}

#[derive(Clone, Debug)]
pub enum DescriptorSet {
    Emulated {
        pool: Arc<RwLock<DescriptorPoolInner>>,
        layouts: Arc<Vec<pso::DescriptorSetLayoutBinding>>,
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

    pub(crate) fn resolve<R: hal::range::RangeArg<u64>>(&self, range: &R) -> Range<u64> {
        *range.start().unwrap_or(&0) .. *range.end().unwrap_or(&self.size)
    }
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}

#[derive(Debug)]
pub(crate) enum MemoryHeap {
    Private,
    Public(hal::MemoryTypeId, metal::Buffer),
    Native(metal::Heap),
}

#[derive(Debug)]
pub struct UnboundBuffer {
    pub(crate) size: u64,
    pub(crate) usage: hal::buffer::Usage,
}
unsafe impl Send for UnboundBuffer {}
unsafe impl Sync for UnboundBuffer {}

#[derive(Debug)]
pub struct UnboundImage {
    pub(crate) texture_desc: metal::TextureDescriptor,
    pub(crate) format: hal::format::Format,
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
