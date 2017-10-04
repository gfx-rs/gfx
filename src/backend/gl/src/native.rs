
use conv;
use core::{self, pass};
use core::target::{Layer, Level};
use core::image as i;
use gl;
use Backend;
use std::cell::Cell;


pub type Shader      = gl::types::GLuint;
pub type Program     = gl::types::GLuint;
pub type FrameBuffer = gl::types::GLuint;
pub type Surface     = gl::types::GLuint;
pub type Texture     = gl::types::GLuint;
pub type Sampler     = gl::types::GLuint;

#[derive(Debug)]
pub struct Buffer {
    pub raw: gl::types::GLuint,
    pub target: gl::types::GLenum,
}

#[derive(Debug)]
pub struct Fence(pub Cell<gl::types::GLsync>);
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

impl Fence {
    pub fn new(sync: gl::types::GLsync) -> Self {
        Fence(Cell::new(sync))
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceView {
    pub object: Texture,
    pub(crate) bind: gl::types::GLenum,
    pub(crate) owned: bool,
}

impl ResourceView {
    pub fn new_texture(t: Texture, kind: i::Kind) -> ResourceView {
        ResourceView {
            object: t,
            bind: conv::image_kind_to_gl(kind),
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


#[derive(Clone, Debug, Copy)]
pub struct GraphicsPipeline {
    pub program: Program,
}

#[derive(Clone, Debug, Copy)]
pub struct ComputePipeline {
    pub program: Program,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Image {
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
pub enum TargetView {
    Surface(Surface),
    Texture(Texture, Level),
    TextureLayer(Texture, Level, Layer),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorSetLayout;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DescriptorSet;

pub struct DescriptorPool {}

impl core::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        layouts.iter().map(|_| DescriptorSet).collect()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[derive(Clone, Copy, Debug, Hash)]
pub struct ShaderModule {
    pub raw: Shader,
}

#[derive(Debug)]
pub struct Memory;

#[derive(Debug)]
pub struct RenderPass {
    pub attachments: Vec<pass::Attachment>,
    pub subpasses: Vec<SubpassDesc>,
}

#[derive(Debug)]
pub struct SubpassDesc {
    pub color_attachments: Vec<usize>,
}

#[derive(Debug)]
pub struct ConstantBufferView;
#[derive(Debug)]
pub struct ShaderResourceView;
#[derive(Debug)]
pub struct UnorderedAccessView;
#[derive(Debug)]
pub struct PipelineLayout;

#[derive(Debug)]
// No inter-queue synchronization required for GL.
pub struct Semaphore;
