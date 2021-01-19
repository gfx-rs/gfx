use crate::{Backend, GlContext, MAX_TEXTURE_SLOTS};

use hal::{
    buffer, format, image as i,
    memory::{Properties, Requirements},
    pass, pso, window as w,
};

use std::{borrow::Borrow, ops::Range, sync::Arc};

pub type TextureTarget = u32;
pub type TextureFormat = u32;
pub type DataType = u32;

// TODO: Consider being generic over `glow::Context` instead
pub type VertexArray = <GlContext as glow::HasContext>::VertexArray;
pub type RawBuffer = <GlContext as glow::HasContext>::Buffer;
pub type Shader = <GlContext as glow::HasContext>::Shader;
pub type Program = <GlContext as glow::HasContext>::Program;
pub type Renderbuffer = <GlContext as glow::HasContext>::Renderbuffer;
pub type RawFramebuffer = <GlContext as glow::HasContext>::Framebuffer;
pub type Texture = <GlContext as glow::HasContext>::Texture;
pub type Sampler = <GlContext as glow::HasContext>::Sampler;
// TODO: UniformLocation was copy in glow 0.3, but in 0.4 it isn't. Wrap it in a Starc for now
// to make it `Sync + Send` instead.
pub type UniformLocation = crate::Starc<<GlContext as glow::HasContext>::UniformLocation>;
pub type DescriptorSetLayout = Arc<Vec<pso::DescriptorSetLayoutBinding>>;

#[derive(Clone, Debug)]
pub struct Framebuffer {
    pub(crate) raw: RawFramebuffer,
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
        match *self {
            Buffer::Unbound { .. } => panic!("Expected bound buffer!"),
            Buffer::Bound { buffer, ref range } => (buffer, range.clone()),
        }
    }
}

#[derive(Debug)]
pub struct BufferView;

#[derive(Debug)]
pub enum Fence {
    Idle { signaled: bool },
    Pending(<GlContext as glow::HasContext>::Fence),
}

unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum BindingRegister {
    Textures,
    UniformBuffers,
    StorageBuffers,
}

/// For each texture in the pipeline layout, store the index of the only
/// sampler (in this layout) that the texture is used with.    
pub(crate) type SamplerBindMap = [Option<u8>; MAX_TEXTURE_SLOTS];

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
    pub(crate) sampler_map: SamplerBindMap,
}

#[derive(Clone, Debug)]
pub struct ComputePipeline {
    pub(crate) program: Program,
    pub(crate) sampler_map: SamplerBindMap,
}

#[derive(Copy, Clone, Debug)]
pub struct Image {
    pub(crate) object_type: ImageType,
    pub(crate) kind: i::Kind,
    pub(crate) format_desc: format::FormatDesc,
    // Required for clearing operations
    pub(crate) channel: format::ChannelType,
    pub(crate) requirements: Requirements,
    pub(crate) num_levels: i::Level,
    pub(crate) num_layers: i::Layer,
}

impl Image {
    pub(crate) fn pitches(&self, level: i::Level) -> [buffer::Offset; 4] {
        let extent = self.kind.extent().at_level(level);
        let bytes_per_texel = self.format_desc.bits as i::Size >> 3;
        let row_pitch = extent.width * bytes_per_texel;
        let depth_pitch = extent.height * row_pitch;
        let array_pitch = extent.depth * depth_pitch;
        [
            bytes_per_texel as _,
            row_pitch as _,
            depth_pitch as _,
            array_pitch as _,
        ]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ImageType {
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
    Renderbuffer {
        raw: Renderbuffer,
        aspects: format::Aspects,
    },
    Texture {
        target: TextureTarget,
        raw: Texture,
        is_3d: bool,
        sub: i::SubresourceRange,
    },
}

impl ImageView {
    pub(crate) fn aspects(&self) -> format::Aspects {
        match *self {
            ImageView::Renderbuffer { aspects, .. } => aspects,
            ImageView::Texture { ref sub, .. } => sub.aspects,
        }
    }
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
        extent: w::Extent2D,
        channel: format::ChannelType,
    ) -> Self {
        SwapchainImage {
            image: Image {
                object_type: ImageType::Renderbuffer {
                    raw: renderbuffer,
                    format,
                },
                channel,
                kind: i::Kind::D2(extent.width as u32, extent.height as u32, 1, 1),
                format_desc: format::FormatDesc {
                    bits: 0,
                    dim: (0, 0),
                    packed: false,
                    aspects: format::Aspects::empty(),
                },
                requirements: Requirements {
                    size: 0,
                    alignment: 1,
                    type_mask: 0,
                },
                num_levels: 1,
                num_layers: 1,
            },
            view: ImageView::Renderbuffer {
                raw: renderbuffer,
                aspects: format::Aspects::COLOR,
            },
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum DescSetBindings {
    Buffer {
        register: BindingRegister,
        buffer: RawBuffer,
        offset: i32,
        size: i32,
    },
    Texture(Texture, TextureTarget),
    Sampler(Sampler),
    SamplerDesc(i::SamplerDesc),
}

#[derive(Clone, Debug)]
pub struct DescriptorSet {
    pub(crate) layout: DescriptorSetLayout,
    //TODO: use `UnsafeCell` instead
    pub(crate) bindings: Vec<DescSetBindings>,
}

#[derive(Debug)]
pub struct DescriptorPool {}

impl pso::DescriptorPool<Backend> for DescriptorPool {
    unsafe fn allocate_one(
        &mut self,
        layout: &DescriptorSetLayout,
    ) -> Result<DescriptorSet, pso::AllocationError> {
        Ok(DescriptorSet {
            layout: Arc::clone(layout),
            bindings: Vec::new(),
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
    pub(crate) emulate_map_allocation: Option<*mut u8>,
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
    pub(crate) fn _attachment_using(&self, at_id: pass::AttachmentId) -> Option<u32> {
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
pub(crate) struct PipelineLayoutSet {
    pub(crate) layout: DescriptorSetLayout,
    /// Mapping of resources, indexed by `pso::DescriptorBinding`, into the whole layout space.
    /// For image resources, the value is the texture slot index.
    /// For sampler resources, the value is the index of the sampler in the whole layout.
    /// For buffers, the value is the uniform or storage slot index.
    /// For unused bindings, the value is `!0`
    pub(crate) bindings: Vec<u8>,
}

#[derive(Debug)]
pub struct PipelineLayout {
    /// Resource mapping for descriptor sets.
    pub(crate) sets: Vec<PipelineLayoutSet>,
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
