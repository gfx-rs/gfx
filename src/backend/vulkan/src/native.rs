use ash::vk;
use core::pso;
use std::collections::BTreeMap;

#[derive(Debug, Hash)]
pub struct Semaphore(pub vk::Semaphore);

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Fence(pub vk::Fence);

#[derive(Debug, Hash)]
pub struct GraphicsPipeline(pub vk::Pipeline);

#[derive(Debug, Hash)]
pub struct ComputePipeline(pub vk::Pipeline);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Buffer {
    pub raw: vk::Buffer,
    pub memory: vk::DeviceMemory,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Image {
    pub raw: vk::Image,
    pub bytes_per_texel: u8,
    pub extent: vk::Extent3D,
}

#[derive(Debug, Hash)]
pub struct RenderPass {
    pub raw: vk::RenderPass,
}

#[derive(Debug, Hash)]
pub struct FrameBuffer {
    pub raw: vk::Framebuffer,
}

#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub raw: vk::DescriptorSetLayout,
}

#[derive(Debug, Hash)]
pub struct PipelineLayout {
    pub raw: vk::PipelineLayout,
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct ShaderLib {
    // TODO: merge SPIR-V modules
    pub shaders: BTreeMap<pso::EntryPoint, vk::ShaderModule>,
}
