use Backend;
use internal::Channel;
use window::SwapchainInner;

use std::collections::HashMap;
use std::ops::{Deref, Range};
use std::os::raw::{c_void, c_long};
use std::sync::{Arc, Condvar, Mutex, RwLock, RwLockReadGuard};

use hal::{self, image, pso};
use hal::backend::FastHashMap;
use hal::format::{Aspects, Format, FormatDesc};

use cocoa::foundation::{NSUInteger};
use metal;
use spirv_cross::{msl, spirv};

use range_alloc::RangeAllocator;


/// Shader module can be compiled in advance if it's resource bindings do not
/// depend on pipeline layout, in which case the value would become `Compiled`.
#[derive(Debug)]
pub enum ShaderModule {
    Compiled {
        library: metal::Library,
        entry_point_map: FastHashMap<String, spirv::EntryPoint>,
    },
    Raw(Vec<u8>),
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
    pub frame: Option<Frame>,
}

#[derive(Clone, Debug)]
pub struct FramebufferInner {
    pub extent: image::Extent,
    pub aspects: Aspects,
    pub colors: Vec<ColorAttachment>,
    pub depth_stencil: Option<metal::MTLPixelFormat>,
}

#[derive(Debug)]
pub struct Framebuffer {
    pub(crate) descriptor: metal::RenderPassDescriptor,
    pub(crate) inner: FramebufferInner,
}

unsafe impl Send for Framebuffer {}
unsafe impl Sync for Framebuffer {}

#[derive(Debug)]
pub struct PipelineLayout {
    // First vertex buffer index to be used by attributes
    pub(crate) attribute_buffer_index: u32,
    pub(crate) res_overrides: HashMap<msl::ResourceBindingLocation, msl::ResourceBinding>,
}

#[derive(Clone, Debug)]
pub struct RasterizerState {
    //TODO: more states
    pub depth_clip: metal::MTLDepthClipMode,
    pub depth_bias: pso::DepthBias,
}

impl Default for RasterizerState {
    fn default() -> Self {
        RasterizerState {
            depth_clip: metal::MTLDepthClipMode::Clip,
            depth_bias: Default::default(),
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

#[derive(Clone, Debug)]
pub struct DepthStencilState {
    pub depth_stencil_desc: Option<pso::DepthStencilDesc>,
    pub depth_stencil_desc_raw: Option<metal::DepthStencilDescriptor>,
    pub depth_stencil_static: Option<metal::DepthStencilState>,
    pub stencil: StencilState<pso::State<pso::StencilValue>>,
}

impl Default for DepthStencilState {
    fn default() -> Self {
        DepthStencilState {
            depth_stencil_desc: None,
            depth_stencil_desc_raw: None,
            depth_stencil_static: None,
            stencil: StencilState::<pso::State<pso::StencilValue>> {
                front_reference: pso::State::Static(0),
                back_reference: pso::State::Static(0),
                front_read_mask: pso::State::Static(!0),
                back_read_mask: pso::State::Static(!0),
                front_write_mask: pso::State::Static(!0),
                back_write_mask: pso::State::Static(!0),
            }
        }
    }
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
    pub(crate) depth_stencil_state: DepthStencilState,
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

#[derive(Clone, Debug)]
pub struct Frame {
    pub swapchain: Arc<RwLock<SwapchainInner>>,
    pub index: hal::FrameImage,
}

#[derive(Clone, Debug)]
pub enum ImageRoot {
    Texture(metal::Texture),
    Frame(Frame),
}

pub enum ImageRootRef<'a> {
    Texture(&'a metal::TextureRef),
    Frame {
        swapchain: RwLockReadGuard<'a, SwapchainInner>,
        index: hal::FrameImage,
    },
}

impl<'a> Deref for ImageRootRef<'a> {
    type Target = metal::TextureRef;
    fn deref(&self) -> &Self::Target {
        match *self {
            ImageRootRef::Texture(tex) => tex,
            ImageRootRef::Frame { ref swapchain, index } => &swapchain[index],
        }
    }
}

impl ImageRoot {
    pub fn resolve(&self) -> ImageRootRef {
        match *self {
            ImageRoot::Texture(ref tex) => ImageRootRef::Texture(tex),
            ImageRoot::Frame(ref frame) => ImageRootRef::Frame {
                swapchain: frame.swapchain.read().unwrap(),
                index: frame.index,
            },
        }
    }
}

#[derive(Debug)]
pub struct Image {
    pub(crate) root: ImageRoot,
    pub(crate) extent: image::Extent,
    pub(crate) num_layers: Option<image::Layer>,
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
        Self::pitches_impl(self.extent.at_level(level), self.format_desc)
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
    pub(crate) root: ImageRoot,
    pub(crate) mtl_format: metal::MTLPixelFormat,
}

unsafe impl Send for ImageView {}
unsafe impl Sync for ImageView {}

#[derive(Debug)]
pub struct Sampler(pub(crate) metal::SamplerState);

unsafe impl Send for Sampler {}
unsafe impl Sync for Sampler {}

#[derive(Debug)]
pub struct Semaphore(pub(crate) *mut c_void);

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

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
    Emulated,
    ArgumentBuffer {
        buffer: metal::Buffer,
        range_allocator: RangeAllocator<NSUInteger>,
    }
}
//TODO: re-evaluate Send/Sync here
unsafe impl Send for DescriptorPool {}
unsafe impl Sync for DescriptorPool {}

impl hal::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_set(&mut self, layout: &DescriptorSetLayout) -> Result<DescriptorSet, pso::AllocationError> {
        match *self {
            DescriptorPool::Emulated => {
                let (layout_bindings, immutable_samplers) = match layout {
                    &DescriptorSetLayout::Emulated(ref bindings, ref samplers) => (bindings, samplers),
                    _ => return Err(pso::AllocationError::IncompatibleLayout),
                };
                let mut sampler_offset = 0;

                // Assume some reasonable starting capacity
                let mut bindings = Vec::with_capacity(layout_bindings.len());

                for layout in layout_bindings.iter() {
                    let binding = match layout.ty {
                        pso::DescriptorType::Sampler => {
                            DescriptorSetBinding::Sampler(if layout.immutable_samplers {
                                let slice = &immutable_samplers[sampler_offset.. sampler_offset + layout.count];
                                sampler_offset += layout.count;
                                slice
                                    .iter()
                                    .map(|s| Some(s.clone()))
                                    .collect()
                            } else {
                                vec![None; layout.count]
                            })
                        }
                        pso::DescriptorType::CombinedImageSampler => {
                            DescriptorSetBinding::Combined(if layout.immutable_samplers {
                                let slice = &immutable_samplers[sampler_offset.. sampler_offset + layout.count];
                                sampler_offset += layout.count;
                                slice
                                    .iter()
                                    .map(|s| (None, Some(s.clone())))
                                    .collect()
                            } else {
                                vec![(None, None); layout.count]
                            })
                        }
                        pso::DescriptorType::SampledImage |
                        pso::DescriptorType::StorageImage |
                        pso::DescriptorType::UniformTexelBuffer |
                        pso::DescriptorType::StorageTexelBuffer |
                        pso::DescriptorType::InputAttachment => {
                            DescriptorSetBinding::Image(vec![None; layout.count])
                        }
                        pso::DescriptorType::UniformBuffer |
                        pso::DescriptorType::StorageBuffer => {
                            DescriptorSetBinding::Buffer(vec![BufferBinding { base: None, dynamic: false }; layout.count])
                        }
                        pso::DescriptorType::UniformBufferDynamic |
                        pso::DescriptorType::StorageBufferDynamic => {
                            DescriptorSetBinding::Buffer(vec![BufferBinding { base: None, dynamic: true }; layout.count])
                        }
                    };

                    let layout_binding = layout.binding as usize;

                    if bindings.len() <= layout_binding {
                        bindings.resize(layout_binding + 1, None);
                    }

                    bindings[layout_binding] = Some(binding);
                }

                // The set may be held onto for a long time, so attempt to shrink to avoid large overallocations
                bindings.shrink_to_fit();

                let inner = DescriptorSetInner {
                    layout: layout_bindings.to_vec(),
                    bindings,
                };
                Ok(DescriptorSet::Emulated(Arc::new(Mutex::new(inner))))
            }
            DescriptorPool::ArgumentBuffer { ref buffer, ref mut range_allocator, } => {
                let (encoder, stage_flags) = match layout {
                    &DescriptorSetLayout::ArgumentBuffer(ref encoder, stages) => (encoder, stages),
                    _ => return Err(pso::AllocationError::IncompatibleLayout),
                };
                range_allocator.allocate_range(encoder.encoded_length()).map(|range| {
                    DescriptorSet::ArgumentBuffer {
                        buffer: buffer.clone(),
                        offset: range.start,
                        encoder: encoder.clone(),
                        stage_flags,
                    }
                }).ok_or(pso::AllocationError::OutOfPoolMemory)
            }
        }
    }

    fn free_sets(&mut self, descriptor_sets: &[DescriptorSet]) {
        match self {
            DescriptorPool::Emulated => {
                return; // Does nothing!  No metal allocation happened here.
            },
            DescriptorPool::ArgumentBuffer {
                ref mut range_allocator,
                ..
            } => {
                for descriptor_set in descriptor_sets {
                    match descriptor_set {
                        DescriptorSet::Emulated(..) => panic!("Tried to free a DescriptorSet not given out by this DescriptorPool!"),
                        DescriptorSet::ArgumentBuffer {
                            offset,
                            encoder,
                            ..
                        } => {
                            let handle_range = (*offset)..offset + encoder.encoded_length();
                            range_allocator.free_range(handle_range);
                        },
                    }
                }
            },
        }
    }

    fn reset(&mut self) {
        match self {
            DescriptorPool::Emulated => {/* No action necessary */}
            DescriptorPool::ArgumentBuffer {
                range_allocator,
                ..
            } => {
                range_allocator.reset();
            }
        }
    }
}

#[derive(Debug)]
pub enum DescriptorSetLayout {
    Emulated(Vec<pso::DescriptorSetLayoutBinding>, Vec<metal::SamplerState>),
    ArgumentBuffer(metal::ArgumentEncoder, pso::ShaderStageFlags),
}
unsafe impl Send for DescriptorSetLayout {}
unsafe impl Sync for DescriptorSetLayout {}

#[derive(Clone, Debug)]
pub enum DescriptorSet {
    Emulated(Arc<Mutex<DescriptorSetInner>>),
    ArgumentBuffer {
        buffer: metal::Buffer,
        offset: NSUInteger,
        encoder: metal::ArgumentEncoder,
        stage_flags: pso::ShaderStageFlags,
    }
}
unsafe impl Send for DescriptorSet {}
unsafe impl Sync for DescriptorSet {}

#[derive(Debug)]
pub struct DescriptorSetInner {
    pub(crate) layout: Vec<pso::DescriptorSetLayoutBinding>, // TODO: maybe don't clone?
    // The index of `bindings` is `pso::DescriptorBinding`
    pub(crate) bindings: Vec<Option<DescriptorSetBinding>>,
}
unsafe impl Send for DescriptorSetInner {}

#[derive(Clone, Debug)]
pub struct BufferBinding {
    pub base: Option<(metal::Buffer, u64)>,
    pub dynamic: bool,
}

#[derive(Clone, Debug)]
pub enum DescriptorSetBinding {
    Sampler(Vec<Option<metal::SamplerState>>),
    Image(Vec<Option<(ImageRoot, image::Layout)>>),
    Combined(Vec<(Option<(ImageRoot, image::Layout)>, Option<metal::SamplerState>)>),
    Buffer(Vec<BufferBinding>),
    //InputAttachment(Vec<(metal::Texture, image::Layout)>),
}

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
    pub(crate) extent: image::Extent,
    pub(crate) num_layers: Option<image::Layer>,
    pub(crate) mip_sizes: Vec<u64>,
    pub(crate) host_visible: bool,
}
unsafe impl Send for UnboundImage {}
unsafe impl Sync for UnboundImage {}

#[derive(Debug)]
pub struct FenceInner {
    pub(crate) mutex: Mutex<bool>,
    pub(crate) condvar: Condvar,
}

pub type Fence = Arc<FenceInner>;

extern "C" {
    #[allow(dead_code)]
    pub fn dispatch_semaphore_wait(
        semaphore: *mut c_void,
        timeout: u64,
    ) -> c_long;

    pub fn dispatch_semaphore_signal(
        semaphore: *mut c_void,
    ) -> c_long;

    pub fn dispatch_semaphore_create(
        value: c_long,
    ) -> *mut c_void;

    pub fn dispatch_release(
        object: *mut c_void,
    );
}
