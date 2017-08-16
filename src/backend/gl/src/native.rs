
use core::target::{Layer, Level};
use core::texture as t;
use texture;
use gl;

pub type Buffer      = gl::types::GLuint;
pub type Shader      = gl::types::GLuint;
pub type Program     = gl::types::GLuint;
pub type FrameBuffer = gl::types::GLuint;
pub type Surface     = gl::types::GLuint;
pub type Texture     = gl::types::GLuint;
pub type Sampler     = gl::types::GLuint;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Fence(pub gl::types::GLsync);
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceView {
    pub(crate) object: Texture,
    pub(crate) bind: gl::types::GLenum,
    pub(crate) owned: bool,
}

impl ResourceView {
    pub fn new_texture(t: Texture, kind: t::Kind) -> ResourceView {
        ResourceView {
            object: t,
            bind: texture::kind_to_gl(kind),
            owned: false,
        }
    }
    pub fn new_buffer(b: Texture) -> ResourceView {
        ResourceView {
            object: b,
            bind: gl::TEXTURE_BUFFER,
            owned: true,
        }
    }
}


#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PipelineState {
    /*
    program: Program,
    primitive: c::Primitive,
    input: Vec<Option<BufferElement>>,
    scissor: bool,
    rasterizer: s::Rasterizer,
    output: OutputMerger,
    */
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Image {
    Surface(Surface),
    Texture(Texture),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct FatSampler {
    pub(crate) object: Sampler,
    pub(crate) info: t::SamplerInfo,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TargetView {
    Surface(Surface),
    Texture(Texture, Level),
    TextureLayer(Texture, Level, Layer),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorHeap;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorSetLayout;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorSet;
