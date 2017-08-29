
use conv;
use core;
use core::target::{Layer, Level};
use core::image as i;
use gl;
use Backend;
use std::cell::Cell;

pub type Buffer      = gl::types::GLuint;
pub type Shader      = gl::types::GLuint;
pub type Program     = gl::types::GLuint;
pub type FrameBuffer = gl::types::GLuint;
pub type Surface     = gl::types::GLuint;
pub type Texture     = gl::types::GLuint;
pub type Sampler     = gl::types::GLuint;

#[derive(Debug)]
pub struct Fence(pub Cell<gl::types::GLsync>);
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceView {
    pub(crate) object: Texture,
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
    program: Program,
}

#[derive(Clone, Debug, Copy)]
pub struct ComputePipeline {
    program: Program,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Image {
    Surface(Surface),
    Texture(Texture),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct FatSampler {
    pub(crate) object: Sampler,
    pub(crate) info: i::SamplerInfo,
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

#[allow(missing_copy_implementations)]
pub struct DescriptorPool {}

impl core::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        unimplemented!()
    }

    fn reset(&mut self) {
        unimplemented!()
    }
}

#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct Heap;
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct ShaderLib;
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct RenderPass;
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct ConstantBufferView;
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct ShaderResourceView;
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct UnorderedAccessView;
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct RenderTargetView {
    pub view: TargetView,
}
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct DepthStencilView;
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct PipelineLayout;
#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub struct Semaphore;
