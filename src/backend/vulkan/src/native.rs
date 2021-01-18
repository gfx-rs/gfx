use crate::{Backend, RawDevice, ROUGH_MAX_ATTACHMENT_COUNT};
use ash::{version::DeviceV1_0, vk};
use hal::{
    device::OutOfMemory,
    image::{Extent, SubresourceRange},
    pso,
};
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Hash)]
pub struct Semaphore(pub vk::Semaphore);

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Fence(pub vk::Fence);

#[derive(Debug, Hash)]
pub struct Event(pub vk::Event);

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
    pub(crate) ty: vk::ImageType,
    pub(crate) flags: vk::ImageCreateFlags,
    pub(crate) extent: vk::Extent3D,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct ImageView {
    pub(crate) image: vk::Image,
    pub(crate) raw: vk::ImageView,
    pub(crate) range: SubresourceRange,
}

#[derive(Debug, Hash)]
pub struct Sampler(pub vk::Sampler);

#[derive(Debug, Hash)]
pub struct RenderPass {
    pub raw: vk::RenderPass,
    pub attachment_count: usize,
}

pub type FramebufferKey = SmallVec<[vk::ImageView; ROUGH_MAX_ATTACHMENT_COUNT]>;

#[derive(Debug)]
pub enum Framebuffer {
    ImageLess(vk::Framebuffer),
    Legacy {
        name: String,
        map: Mutex<HashMap<FramebufferKey, vk::Framebuffer>>,
        extent: Extent,
    },
}

#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub(crate) raw: vk::DescriptorSetLayout,
    pub(crate) bindings: Arc<Vec<pso::DescriptorSetLayoutBinding>>,
}

#[derive(Debug)]
pub struct DescriptorSet {
    pub(crate) raw: vk::DescriptorSet,
    pub(crate) bindings: Arc<Vec<pso::DescriptorSetLayoutBinding>>,
}

#[derive(Debug, Hash)]
pub struct PipelineLayout {
    pub(crate) raw: vk::PipelineLayout,
}

#[derive(Debug)]
pub struct PipelineCache {
    pub(crate) raw: vk::PipelineCache,
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct ShaderModule {
    pub(crate) raw: vk::ShaderModule,
}

#[derive(Debug)]
pub struct DescriptorPool {
    pub(crate) raw: vk::DescriptorPool,
    pub(crate) device: Arc<RawDevice>,
    /// This vec only exists to re-use allocations when `DescriptorSet`s are freed.
    pub(crate) set_free_vec: Vec<vk::DescriptorSet>,
}

impl pso::DescriptorPool<Backend> for DescriptorPool {
    unsafe fn allocate_one(
        &mut self,
        layout: &DescriptorSetLayout,
    ) -> Result<DescriptorSet, pso::AllocationError> {
        let raw_layouts = [layout.raw];
        let info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.raw)
            .set_layouts(&raw_layouts);

        self.device
            .raw
            .allocate_descriptor_sets(&info)
            //Note: https://github.com/MaikKlein/ash/issues/358
            .map(|mut sets| DescriptorSet {
                raw: sets.pop().unwrap(),
                bindings: Arc::clone(&layout.bindings),
            })
            .map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
                    pso::AllocationError::OutOfMemory(OutOfMemory::Host)
                }
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                    pso::AllocationError::OutOfMemory(OutOfMemory::Device)
                }
                vk::Result::ERROR_OUT_OF_POOL_MEMORY => pso::AllocationError::OutOfPoolMemory,
                _ => pso::AllocationError::FragmentedPool,
            })
    }

    unsafe fn allocate<'a, I, E>(
        &mut self,
        layout_intoiter: I,
        list: &mut E,
    ) -> Result<(), pso::AllocationError>
    where
        I: IntoIterator<Item = &'a DescriptorSetLayout>,
        I::IntoIter: ExactSizeIterator,
        E: Extend<DescriptorSet>,
    {
        let layouts_iter = layout_intoiter.into_iter();
        let mut raw_layouts = Vec::with_capacity(layouts_iter.len());
        let mut layout_bindings = Vec::with_capacity(layouts_iter.len());
        for layout in layouts_iter {
            raw_layouts.push(layout.raw);
            layout_bindings.push(Arc::clone(&layout.bindings));
        }

        let info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.raw)
            .set_layouts(&raw_layouts);

        self.device
            .raw
            .allocate_descriptor_sets(&info)
            .map(|sets| {
                list.extend(
                    sets.into_iter()
                        .zip(layout_bindings)
                        .map(|(raw, bindings)| DescriptorSet { raw, bindings }),
                )
            })
            .map_err(|err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
                    pso::AllocationError::OutOfMemory(OutOfMemory::Host)
                }
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                    pso::AllocationError::OutOfMemory(OutOfMemory::Device)
                }
                vk::Result::ERROR_OUT_OF_POOL_MEMORY => pso::AllocationError::OutOfPoolMemory,
                _ => pso::AllocationError::FragmentedPool,
            })
    }

    unsafe fn free<I>(&mut self, descriptor_sets: I)
    where
        I: IntoIterator<Item = DescriptorSet>,
    {
        self.set_free_vec.clear();
        self.set_free_vec
            .extend(descriptor_sets.into_iter().map(|d| d.raw));
        self.device
            .raw
            .free_descriptor_sets(self.raw, &self.set_free_vec);
    }

    unsafe fn reset(&mut self) {
        assert_eq!(
            Ok(()),
            self.device
                .raw
                .reset_descriptor_pool(self.raw, vk::DescriptorPoolResetFlags::empty())
        );
    }
}

#[derive(Debug, Hash)]
pub struct QueryPool(pub vk::QueryPool);
