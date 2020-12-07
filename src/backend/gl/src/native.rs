use crate::{Backend, GlContext};

use auxil::FastHashMap;
use hal::{
    buffer, format, image as i,
    memory::{Properties, Requirements},
    pass, pso,
};

use parking_lot::{Mutex, RwLock};

use std::{borrow::Borrow, cell::Cell, ops::Range, sync::Arc};

pub type TextureTarget = u32;
pub type TextureFormat = u32;
pub type DataType = u32;

// TODO: Consider being generic over `glow::Context` instead
pub type VertexArray = <GlContext as glow::HasContext>::VertexArray;
pub type RawBuffer = <GlContext as glow::HasContext>::Buffer;
pub type Shader = <GlContext as glow::HasContext>::Shader;
pub type Program = <GlContext as glow::HasContext>::Program;
pub type Renderbuffer = <GlContext as glow::HasContext>::Renderbuffer;
pub type Texture = <GlContext as glow::HasContext>::Texture;
pub type Sampler = <GlContext as glow::HasContext>::Sampler;
// TODO: UniformLocation was copy in glow 0.3, but in 0.4 it isn't. Wrap it in a Starc for now
// to make it `Sync + Send` instead.
pub type UniformLocation = crate::Starc<<GlContext as glow::HasContext>::UniformLocation>;
pub type DescriptorSetLayout = Arc<Vec<pso::DescriptorSetLayoutBinding>>;

pub type RawFrameBuffer = <GlContext as glow::HasContext>::Framebuffer;

#[derive(Clone, Debug)]
pub struct FrameBuffer {
    pub(crate) fbos: Vec<Option<RawFrameBuffer>>,
}

#[derive(Debug)]
pub enum Buffer {
    Unbound {
        size: buffer::Offset,
        usage: buffer::Usage,
    },
    Bound {
        buffer: RawBuffer,
        range: Range<buffer::Offset>,
    },
}

impl Buffer {
    // Asserts that the buffer is bound and returns the raw gl buffer along with its sub-range.
    pub(crate) fn as_bound(&self) -> (RawBuffer, Range<u64>) {
        match self {
            Buffer::Unbound { .. } => panic!("Expected bound buffer!"),
            Buffer::Bound { buffer, range, .. } => (*buffer, range.clone()),
        }
    }
}

#[derive(Debug)]
pub struct BufferView;

#[derive(Copy, Clone, Debug)]
pub(crate) enum FenceInner {
    Idle { signaled: bool },
    Pending(Option<<GlContext as glow::HasContext>::Fence>),
}

#[derive(Debug)]
pub struct Fence(pub(crate) Cell<FenceInner>);
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum BindingTypes {
    Images,
    UniformBuffers,
    StorageBuffers,
}

#[derive(Clone, Debug)]
pub struct DescRemapData {
    bindings: FastHashMap<
        (
            BindingTypes,
            pso::DescriptorSetIndex,
            pso::DescriptorBinding,
        ),
        Vec<pso::DescriptorBinding>,
    >,
    next_binding: FastHashMap<BindingTypes, pso::DescriptorBinding>,
}

/// Stores where the descriptor bindings have been remaped too.
///
/// OpenGL doesn't support sets, so we have to flatten out the bindings.
impl DescRemapData {
    pub fn new() -> Self {
        DescRemapData {
            bindings: FastHashMap::default(),
            next_binding: FastHashMap::default(),
        }
    }

    pub fn insert_missing_binding_into_spare(
        &mut self,
        btype: BindingTypes,
        set: pso::DescriptorSetIndex,
        binding: pso::DescriptorBinding,
    ) -> &[pso::DescriptorBinding] {
        let nb = self.next_binding.entry(btype).or_insert(0);
        let val = self
            .bindings
            .entry((btype, set, binding))
            .or_insert(Vec::new());
        val.push(*nb);
        *nb += 1;
        &*val
    }

    pub fn reserve_binding(&mut self, btype: BindingTypes) -> pso::DescriptorBinding {
        let nb = self.next_binding.entry(btype).or_insert(0);
        *nb += 1;
        *nb - 1
    }

    pub fn insert_missing_binding(
        &mut self,
        nb: pso::DescriptorBinding,
        btype: BindingTypes,
        set: pso::DescriptorSetIndex,
        binding: pso::DescriptorBinding,
    ) -> &[pso::DescriptorBinding] {
        let val = self
            .bindings
            .entry((btype, set, binding))
            .or_insert(Vec::new());
        val.push(nb);
        &*val
    }

    pub fn get_binding(
        &self,
        btype: BindingTypes,
        set: pso::DescriptorSetIndex,
        binding: pso::DescriptorBinding,
    ) -> Option<&[pso::DescriptorBinding]> {
        self.bindings.get(&(btype, set, binding)).map(AsRef::as_ref)
    }
}

#[derive(Clone, Debug)]
pub struct GraphicsPipeline {
    pub(crate) program: Program,
    pub(crate) primitive: u32,
    pub(crate) patch_size: Option<i32>,
    pub(crate) blend_targets: Vec<pso::ColorBlendDesc>,
    pub(crate) attributes: Vec<AttributeDesc>,
    pub(crate) vertex_buffers: Vec<Option<pso::VertexBufferDesc>>,
    pub(crate) uniforms: Vec<UniformDesc>,
    pub(crate) rasterizer: pso::Rasterizer,
    pub(crate) depth: Option<pso::DepthTest>,
    pub(crate) baked_states: pso::BakedStates,
}

#[derive(Clone, Debug)]
pub struct ComputePipeline {
    pub(crate) program: Program,
}

#[derive(Copy, Clone, Debug)]
pub struct Image {
    pub(crate) kind: ImageKind,
    // Required for clearing operations
    pub(crate) channel: format::ChannelType,
    pub(crate) requirements: Requirements,
    pub(crate) num_levels: i::Level,
    pub(crate) num_layers: i::Layer,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ImageKind {
    Renderbuffer {
        raw: Renderbuffer,
        format: TextureFormat,
    },
    Texture {
        target: TextureTarget,
        raw: Texture,
        level_count: i::Level,
        layer_count: i::Layer,
        format: TextureFormat,
        pixel_type: DataType,
    },
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
/// Additionally storing the `SamplerDesc` for older OpenGL versions, which
/// don't support separate sampler objects.
pub enum FatSampler {
    Sampler(Sampler),
    Info(i::SamplerDesc),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ImageView {
    Renderbuffer(Renderbuffer),
    Texture {
        target: TextureTarget,
        raw: Texture,
        is_3d: bool,
        sub: i::SubresourceRange,
    },
}

#[derive(Debug)]
pub struct SwapchainImage {
    pub image: Image,
    pub view: ImageView,
}

impl Borrow<Image> for SwapchainImage {
    fn borrow(&self) -> &Image {
        &self.image
    }
}

impl Borrow<ImageView> for SwapchainImage {
    fn borrow(&self) -> &ImageView {
        &self.view
    }
}

impl SwapchainImage {
    #[cfg(not(dummy))]
    pub(crate) fn new(
        renderbuffer: Renderbuffer,
        format: TextureFormat,
        channel: format::ChannelType,
    ) -> Self {
        SwapchainImage {
            image: Image {
                kind: ImageKind::Renderbuffer {
                    raw: renderbuffer,
                    format,
                },
                channel,
                requirements: Requirements {
                    size: 0,
                    alignment: 1,
                    type_mask: 0,
                },
                num_levels: 1,
                num_layers: 1,
            },
            view: ImageView::Renderbuffer(renderbuffer),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum DescSetBindings {
    Buffer {
        ty: BindingTypes,
        binding: pso::DescriptorBinding,
        buffer: RawBuffer,
        offset: i32,
        size: i32,
    },
    Texture(pso::DescriptorBinding, Texture, TextureTarget),
    Sampler(pso::DescriptorBinding, Sampler),
    SamplerDesc(pso::DescriptorBinding, i::SamplerDesc),
}

#[derive(Clone, Debug)]
pub struct DescriptorSet {
    pub(crate) layout: DescriptorSetLayout,
    pub(crate) bindings: Arc<Mutex<Vec<DescSetBindings>>>,
}

#[derive(Debug)]
pub struct DescriptorPool {}

impl pso::DescriptorPool<Backend> for DescriptorPool {
    unsafe fn allocate_set(
        &mut self,
        layout: &DescriptorSetLayout,
    ) -> Result<DescriptorSet, pso::AllocationError> {
        Ok(DescriptorSet {
            layout: Arc::clone(layout),
            bindings: Arc::new(Mutex::new(Vec::new())),
        })
    }

    unsafe fn free<I>(&mut self, descriptor_sets: I)
    where
        I: IntoIterator<Item = DescriptorSet>,
    {
        for _set in descriptor_sets {
            // Poof!  Does nothing, because OpenGL doesn't have a meaningful concept of a `DescriptorSet`.
        }
    }

    unsafe fn reset(&mut self) {
        // Poof!  Does nothing, because OpenGL doesn't have a meaningful concept of a `DescriptorSet`.
    }
}

#[derive(Debug)]
pub enum ShaderModule {
    Raw(Shader),
    Spirv(Vec<u32>),
    #[cfg(feature = "naga")]
    Naga(naga::Module, Vec<u32>),
}

#[derive(Debug)]
pub struct Memory {
    pub(crate) properties: Properties,
    /// Gl buffer and the target that should be used for map operations.  Image memory is faked and
    /// has no associated buffer, so this will be None for image memory.
    pub(crate) buffer: Option<(RawBuffer, u32)>,
    /// Allocation size
    pub(crate) size: u64,
    pub(crate) map_flags: u32,
    pub(crate) emulate_map_allocation: Cell<Option<*mut u8>>,
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}

#[derive(Clone, Debug)]
pub struct RenderPass {
    pub(crate) attachments: Vec<pass::Attachment>,
    pub(crate) subpasses: Vec<SubpassDesc>,
}

#[derive(Clone, Debug)]
pub struct SubpassDesc {
    pub(crate) color_attachments: Vec<usize>,
    pub(crate) depth_stencil: Option<usize>,
}

impl SubpassDesc {
    /// Check if an attachment is used by this sub-pass.
    pub(crate) fn attachment_using(&self, at_id: pass::AttachmentId) -> Option<u32> {
        if self.depth_stencil == Some(at_id) {
            Some(glow::DEPTH_STENCIL_ATTACHMENT)
        } else {
            self.color_attachments
                .iter()
                .position(|id| *id == at_id)
                .map(|p| glow::COLOR_ATTACHMENT0 + p as u32)
        }
    }
}

#[derive(Debug)]
pub struct PipelineLayout {
    pub(crate) desc_remap_data: Arc<RwLock<DescRemapData>>,
}

#[derive(Debug)]
// No inter-queue synchronization required for GL.
pub struct Semaphore;

#[derive(Clone, Debug)]
pub struct AttributeDesc {
    pub(crate) location: u32,
    pub(crate) offset: u32,
    pub(crate) binding: u32,
    pub(crate) size: i32,
    pub(crate) format: u32,
    pub(crate) vertex_attrib_fn: VertexAttribFunction,
}

#[derive(Clone, Debug)]
pub struct UniformDesc {
    pub(crate) location: UniformLocation,
    pub(crate) offset: u32,
    pub(crate) utype: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum VertexAttribFunction {
    Float,   // glVertexAttribPointer
    Integer, // glVertexAttribIPointer
    Double,  // glVertexAttribLPointer
}
