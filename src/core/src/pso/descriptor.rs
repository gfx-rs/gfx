//! Descriptor sets and layouts.

use std::fmt;
use std::ops::Range;

use {Backend};
use image::ImageLayout;
use super::ShaderStageFlags;

///
// TODO: Grasping and remembering the differences between these
//       types is a tough task. We might be able to come up with better names?
//       Or even use tuples to describe functionality instead of coming up with fancy names.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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
    ConstantBuffer,
    /// Read-Write, structured buffer.
    StorageBuffer,
    /// Allows unfiltered loads of pixel local data in the fragment shader.
    InputAttachment,

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
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct DescriptorRangeDesc {
    /// Type of the stored descriptors.
    pub ty: DescriptorType,
    /// Amount of space.
    pub count: usize,
}

///
pub trait DescriptorPool<B: Backend>: Send + fmt::Debug {
    /// Allocate one or multiple descriptor sets from the pool.
    ///
    /// Each descriptor set will be allocated from the pool according to the corresponding set layout.
    /// The descriptor pool _must_ have enough space in to allocate the required descriptors.
    /// Descriptors will become invalid once the pool got reset. Usage of invalidated descriptor sets results
    /// in undefined behavior.
    fn allocate_sets(&mut self, layouts: &[&B::DescriptorSetLayout]) -> Vec<B::DescriptorSet>;

    ///
    fn reset(&mut self);
}

#[allow(missing_docs)] //TODO
pub struct DescriptorSetWrite<'a, 'b, B: Backend> {
    pub set: &'a B::DescriptorSet,
    pub binding: usize,
    pub array_offset: usize,
    pub write: DescriptorWrite<'b, B>,
}

#[allow(missing_docs)] //TODO
pub enum DescriptorWrite<'a, B: Backend> {
    Sampler(Vec<&'a B::Sampler>),
    SampledImage(Vec<(&'a B::ImageView, ImageLayout)>),
    StorageImage(Vec<(&'a B::ImageView, ImageLayout)>),
    InputAttachment(Vec<(&'a B::ImageView, ImageLayout)>),
    UniformBuffer(Vec<(&'a B::Buffer, Range<u64>)>),
    StorageBuffer(Vec<(&'a B::Buffer, Range<u64>)>),
    UniformTexelBuffer(Vec<&'a B::BufferView>),
    StorageTexelBuffer(Vec<&'a B::BufferView>),
}
