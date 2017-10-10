use ash::vk;
use ash::version::DeviceV1_0;
use core;
use core::image::SubresourceRange;
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
    pub(crate) inner: vk::DeviceMemory,
    pub(crate) ptr: *mut u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Buffer {
    pub(crate) raw: vk::Buffer,
    pub(crate) memory: vk::DeviceMemory,
    pub(crate) offset: u64,
    pub(crate) ptr: *mut u8,
}

unsafe impl Sync for Buffer {}
unsafe impl Send for Buffer {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BufferView {
    //TODO: `VkBufferView`
    pub(crate) buffer: vk::Buffer,
    pub(crate) range: Range<u64>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Image {
    pub(crate) raw: vk::Image,
    pub(crate) bytes_per_texel: u8,
    pub(crate) extent: vk::Extent3D,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct ImageView {
    pub(crate) image: vk::Image,
    pub(crate) view: vk::ImageView,
    pub(crate) range: SubresourceRange,
}

#[derive(Debug, Hash)]
pub struct Sampler(pub vk::Sampler);

#[derive(Debug, Hash)]
pub struct RenderPass {
    pub raw: vk::RenderPass,
}

#[derive(Debug, Hash)]
pub struct FrameBuffer {
    pub(crate)  raw: vk::Framebuffer,
}

#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub(crate)  raw: vk::DescriptorSetLayout,
}

#[derive(Debug)]
pub struct DescriptorSet {
    pub(crate)  raw: vk::DescriptorSet,
}

#[derive(Debug, Hash)]
pub struct PipelineLayout {
    pub(crate)  raw: vk::PipelineLayout,
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct ShaderModule {
    pub(crate)  raw: vk::ShaderModule,
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
