//! Descriptor sets and layouts.

use std::borrow::Borrow;
use std::fmt;

use {Backend};
use image::ImageLayout;
use pso::ShaderStageFlags;
use range::RangeArg;


///
// TODO: Grasping and remembering the differences between these
//       types is a tough task. We might be able to come up with better names?
//       Or even use tuples to describe functionality instead of coming up with fancy names.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DescriptorType {
    /// Controls filtering parameters for sampling from images.
    Sampler,
    /// Allows sampling (filtered loading) from associated image memory.
    /// Usually combined with a `Sampler`.
    SampledImage,
    /// Allows atomic operations, (non-filtered) loads and stores on image memory.
    StorageImage,
    /// Read-only, formatted buffer.
    UniformTexelBuffer,
    /// Read-Write, formatted buffer.
    StorageTexelBuffer,
    /// Read-only, structured buffer.
    UniformBuffer,
    /// Read-Write, structured buffer.
    StorageBuffer,
    /// Allows unfiltered loads of pixel local data in the fragment shader.
    InputAttachment,

    ///
    CombinedImageSampler,

    // TODO: Dynamic descriptors
}

/// Binding descriptiong of a descriptor set
///
/// A descriptor set consists of multiple binding points.
/// Each binding point contains one or multiple descriptors of a certain type.
/// The binding point is only valid for the pipelines stages specified.
///
/// The binding _must_ match with the corresponding shader interface.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DescriptorSetLayoutBinding {
    /// Integer identifier of the binding.
    pub binding: usize,
    /// Type of the bound descriptors.
    pub ty: DescriptorType,
    /// Number of descriptors bound.
    pub count: usize,
    /// Valid shader stages.
    pub stage_flags: ShaderStageFlags,

    // TODO: immutable samplers?
}

/// Set of descriptors of a specific type.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DescriptorRangeDesc {
    /// Type of the stored descriptors.
    pub ty: DescriptorType,
    /// Amount of space.
    pub count: usize,
}

///
pub trait DescriptorPool<B: Backend>: Send + Sync + fmt::Debug {
    /// Allocate a descriptor set from the pool.
    ///
    /// The descriptor set will be allocated from the pool according to the corresponding set layout.
    /// The descriptor pool _must_ have enough space in to allocate the required descriptor.
    /// Descriptors will become invalid once the pool is reset. Usage of invalidated descriptor sets results
    /// in undefined behavior.
    fn allocate_set(&mut self, layout: &B::DescriptorSetLayout) -> B::DescriptorSet {
        self.allocate_sets(Some(layout)).remove(0)
    }

    /// Allocate one or multiple descriptor sets from the pool.
    ///
    /// Each descriptor set will be allocated from the pool according to the corresponding set layout.
    /// The descriptor pool _must_ have enough space in to allocate the required descriptors.
    /// Descriptors will become invalid once the pool is reset. Usage of invalidated descriptor sets results
    /// in undefined behavior.
    fn allocate_sets<I>(&mut self, layouts: I) -> Vec<B::DescriptorSet>
    where
        I: IntoIterator,
        I::Item: Borrow<B::DescriptorSetLayout>,
    {
        layouts.into_iter().map(|layout| self.allocate_set(layout.borrow())).collect()
    }

    ///
    fn reset(&mut self);
}

#[allow(missing_docs)] //TODO
pub struct DescriptorSetWrite<'a, B: Backend, R: 'a + RangeArg<u64>> {
    pub set: &'a B::DescriptorSet,
    pub binding: usize,
    pub array_offset: usize,
    pub write: DescriptorWrite<'a, B, R>,
}

#[allow(missing_docs)] //TODO
#[derive(Clone, Copy)]
pub enum DescriptorWrite<'a, B: Backend, R: 'a + RangeArg<u64>> {
    Sampler(&'a [&'a B::Sampler]),
    SampledImage(&'a [(&'a B::ImageView, ImageLayout)]),
    StorageImage(&'a [(&'a B::ImageView, ImageLayout)]),
    InputAttachment(&'a [(&'a B::ImageView, ImageLayout)]),
    UniformBuffer(&'a [(&'a B::Buffer, R)]),
    StorageBuffer(&'a [(&'a B::Buffer, R)]),
    UniformTexelBuffer(&'a [&'a B::BufferView]),
    StorageTexelBuffer(&'a [&'a B::BufferView]),
    CombinedImageSampler(&'a [(&'a B::Sampler, &'a B::ImageView, ImageLayout)]),
}

#[allow(missing_docs)] //TODO
#[derive(Clone, Copy)]
pub struct DescriptorSetCopy<'a, B: Backend> {
    pub src_set: &'a B::DescriptorSet,
    pub src_binding: usize,
    pub src_array_offset: usize,
    pub dst_set: &'a B::DescriptorSet,
    pub dst_binding: usize,
    pub dst_array_offset: usize,
    pub count: usize,
}
