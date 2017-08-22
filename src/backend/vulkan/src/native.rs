use ash::vk;
use ash::version::DeviceV1_0;
use core;
use core::texture::SubresourceRange;
use std::collections::BTreeMap;
use std::sync::Arc;
use {Backend, RawDevice};

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

#[derive(Debug)]
pub struct DescriptorSet {
    pub raw: vk::DescriptorSet,
}

#[derive(Debug, Hash)]
pub struct PipelineLayout {
    pub raw: vk::PipelineLayout,
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct ShaderLib {
    // TODO: merge SPIR-V modules
    pub shaders: BTreeMap<core::pso::EntryPoint, vk::ShaderModule>,
}

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct RenderTargetView {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub range: SubresourceRange
}

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct DepthStencilView {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub range: SubresourceRange
}

#[derive(Debug)]
pub struct DescriptorHeap {
    pub(crate) num_cbv_srv_uav: usize,
    pub(crate) num_sampler: usize,
}

#[derive(Debug)]
pub struct DescriptorPool {
    pub(crate) raw: vk::DescriptorPool,
    device: Arc<RawDevice>,
}

impl core::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        unimplemented!()
    }

    fn reset(&mut self) {
        assert_eq!(Ok(()), unsafe {
            self.device.0.reset_descriptor_pool(
                self.raw,
                vk::DescriptorPoolResetFlags::empty(),
            )
        });
    }
}
