use ash::vk;
use ash::version::DeviceV1_0;
use core;
use core::image::SubresourceLayers;
use std::ops::Range;
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

#[derive(Debug, Hash)]
pub struct Memory {
    pub inner: vk::DeviceMemory,
    pub ptr: *mut u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Buffer {
    pub raw: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub offset: u64,
    pub ptr: *mut u8,
}

unsafe impl Sync for Buffer {}
unsafe impl Send for Buffer {}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Image {
    pub raw: vk::Image,
    pub bytes_per_texel: u8,
    pub extent: vk::Extent3D,
}


#[derive(Debug, Hash)]
pub struct Sampler(pub vk::Sampler);

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
pub struct ShaderModule {
    pub raw: vk::ShaderModule,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ConstantBufferView {
    pub buffer: vk::Buffer,
    pub range: Range<u64>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ShaderResourceView {
    Buffer,
    Image(vk::ImageView),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum UnorderedAccessView {
    Buffer,
    Image(vk::ImageView),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RenderTargetView {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub layers: SubresourceLayers,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DepthStencilView {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub layers: SubresourceLayers,
}

#[derive(Debug)]
pub struct DescriptorPool {
    pub(crate) raw: vk::DescriptorPool,
    pub(crate) device: Arc<RawDevice>,
}

impl core::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets(&mut self, layouts: &[&DescriptorSetLayout]) -> Vec<DescriptorSet> {
        use std::ptr;

        let layouts = layouts.iter().map(|layout| {
            layout.raw
        }).collect::<Vec<_>>();

        let info = vk::DescriptorSetAllocateInfo {
            s_type: vk::StructureType::DescriptorSetAllocateInfo,
            p_next: ptr::null(),
            descriptor_pool: self.raw,
            descriptor_set_count: layouts.len() as u32,
            p_set_layouts: layouts.as_ptr(),
        };

        let descriptor_sets = unsafe {
            self.device.0.allocate_descriptor_sets(&info)
                         .expect("Error on descriptor sets creation") // TODO
        };

        descriptor_sets.into_iter().map(|set| {
            DescriptorSet { raw: set }
        }).collect::<Vec<_>>()
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
