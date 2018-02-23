use std::cell::Cell;

use hal::{self, format, image as i, pass, pso};
use hal::memory::Properties;

use gl;
use Backend;
use std::borrow::Borrow;


pub type RawBuffer   = gl::types::GLuint;
pub type Shader      = gl::types::GLuint;
pub type Program     = gl::types::GLuint;
pub type FrameBuffer = gl::types::GLuint;
pub type Surface     = gl::types::GLuint;
pub type Texture     = gl::types::GLuint;
pub type Sampler     = gl::types::GLuint;

pub const DEFAULT_FRAMEBUFFER: FrameBuffer = 0;

#[derive(Debug)]
pub struct Buffer {
    pub(crate) raw: RawBuffer,
    pub(crate) target: gl::types::GLenum,
}

#[derive(Debug)]
pub struct BufferView;

#[derive(Debug)]
pub struct Fence(pub(crate) Cell<gl::types::GLsync>);
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

impl Fence {
    pub(crate) fn new(sync: gl::types::GLsync) -> Self {
        Fence(Cell::new(sync))
    }
}

#[derive(Clone, Debug)]
pub struct GraphicsPipeline {
    pub(crate) program: Program,
    pub(crate) primitive: gl::types::GLenum,
    pub(crate) patch_size: Option<gl::types::GLint>,
    pub(crate) blend_targets: Vec<pso::ColorBlendDesc>,
    pub(crate) attributes: Vec<AttributeDesc>,
    pub(crate) vertex_buffers: Vec<pso::VertexBufferDesc>,
}

#[derive(Clone, Debug, Copy)]
pub struct ComputePipeline {
    pub(crate) program: Program,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Image {
    pub(crate) kind: ImageKind,
    // Required for clearing operations
    pub(crate) channel: format::ChannelType,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ImageKind {
    Surface(Surface),
    Texture(Texture),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
/// Additionally storing the `SamplerInfo` for older OpenGL versions, which
/// don't support separate sampler objects.
pub enum FatSampler {
    Sampler(Sampler),
    Info(i::SamplerInfo),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ImageView {
    Surface(Surface),
    Texture(Texture, i::Level),
    TextureLayer(Texture, i::Level, i::Layer),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorSetLayout;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorSet;

#[derive(Debug)]
pub struct DescriptorPool {}

impl hal::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets<I>(&mut self, layouts: I) -> Vec<DescriptorSet>
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSetLayout>,
    {
        layouts.into_iter().map(|_| DescriptorSet).collect()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[derive(Clone, Debug, Hash)]
pub enum ShaderModule {
    Raw(Shader),
    Spirv(Vec<u8>),
}

#[derive(Debug)]
pub struct Memory {
    pub(crate) properties: Properties,
    pub(crate) first_bound_buffer: Cell<RawBuffer>,
    /// Allocation size
    pub(crate) size: u64,
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}

impl Memory {
    pub fn can_upload(&self) -> bool {
        self.properties.contains(Properties::CPU_VISIBLE)
    }

    pub fn can_download(&self) -> bool {
        self.properties.contains(Properties::CPU_VISIBLE | Properties::CPU_CACHED)
    }

    pub fn map_flags(&self) -> gl::types::GLenum {
        let mut flags = 0;
        if self.can_download() {
            flags |= gl::MAP_READ_BIT;
        }
        if self.can_upload() {
            flags |= gl::MAP_WRITE_BIT;
        }
        flags
    }
}

#[derive(Clone, Debug)]
pub struct RenderPass {
    pub(crate) attachments: Vec<pass::Attachment>,
    pub(crate) subpasses: Vec<SubpassDesc>,
}

#[derive(Clone, Debug)]
pub struct SubpassDesc {
    pub(crate) color_attachments: Vec<usize>,
}

impl SubpassDesc {
    /// Check if an attachment is used by this sub-pass.
    pub(crate) fn is_using(&self, at_id: pass::AttachmentId) -> bool {
        self.color_attachments.iter()
            .any(|id| *id == at_id)
    }
}

#[derive(Debug)]
pub struct PipelineLayout;

#[derive(Debug)]
// No inter-queue synchronization required for GL.
pub struct Semaphore;

#[derive(Debug, Clone, Copy)]
pub struct AttributeDesc {
    pub(crate) location: gl::types::GLuint,
    pub(crate) offset: u32,
    pub(crate) binding: gl::types::GLuint,
    pub(crate) size: gl::types::GLint,
    pub(crate) format: gl::types::GLenum,
    pub(crate) vertex_attrib_fn: VertexAttribFunction,
}

#[derive(Debug, Clone, Copy)]
pub enum VertexAttribFunction {
    Float, // glVertexAttribPointer
    Integer, // glVertexAttribIPointer
    Double, // glVertexAttribLPointer
}
