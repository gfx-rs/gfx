use ash::vk;
use ash::version::DeviceV1_0;
use hal;
use hal::image::SubresourceRange;
use std::borrow::Borrow;
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
    pub(crate) raw: vk::DeviceMemory,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Buffer {
    pub(crate) raw: vk::Buffer,
}

unsafe impl Sync for Buffer {}
unsafe impl Send for Buffer {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BufferView {
    pub(crate) raw: vk::BufferView,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Image {
    pub(crate) raw: vk::Image,
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
pub struct Framebuffer {
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

impl hal::DescriptorPool<Backend> for DescriptorPool {
    fn allocate_sets<I>(&mut self, layouts: I) -> Vec<DescriptorSet>
    where
        I: IntoIterator,
        I::Item: Borrow<DescriptorSetLayout>,
    {
        use std::ptr;

        let layouts = layouts.into_iter().map(|layout| {
            layout.borrow().raw
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

#[derive(Debug, Hash)]
pub struct QueryPool(pub vk::QueryPool);
