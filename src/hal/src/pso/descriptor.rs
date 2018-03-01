//! Descriptor sets and layouts.
//! A descriptor is an object that describes the connection between a resource, such as
//! an `Image` or `Buffer`, and a variable in a shader. Descriptors are organized into
//! sets, each of which contains multiple descriptors that are bound and unbound to
//! shaders as a single unit. Each descriptor set may contain descriptors to multiple 
//! different sorts of resources, and a shader may use multiple descriptor sets at a time.

use std::borrow::Borrow;
use std::fmt;
use std::ops::Range;

use {Backend};
use buffer::Offset;
use image::ImageLayout;
use pso::ShaderStageFlags;

///
pub type DescriptorSetIndex = u16;
///
pub type DescriptorBinding = u32;
///
pub type DescriptorArrayIndex = usize;

/// DOC TODO: Grasping and remembering the differences between these
///       types is a tough task. We might be able to come up with better names?
///       Or even use tuples to describe functionality instead of coming up with fancy names.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DescriptorType {
    /// Controls filtering parameters for sampling from images.
    Sampler = 0,
    ///
    CombinedImageSampler = 1,
    /// Allows sampling (filtered loading) from associated image memory.
    /// Usually combined with a `Sampler`.
    SampledImage = 2,
    /// Allows atomic operations, (non-filtered) loads and stores on image memory.
    StorageImage = 3,
    /// Read-only, formatted buffer.
    UniformTexelBuffer = 4,
    /// Read-Write, formatted buffer.
    StorageTexelBuffer = 5,
    /// Read-only, structured buffer.
    UniformBuffer = 6,
    /// Read-Write, structured buffer.
    StorageBuffer = 7,
    ///
    UniformBufferDynamic = 8,
    ///
    UniformImageDynamic = 9,
    /// Allows unfiltered loads of pixel local data in the fragment shader.
    InputAttachment = 10,
}

/// Binding description of a descriptor set
///
/// A descriptor set consists of multiple binding points.
/// Each binding point contains one or multiple descriptors of a certain type.
/// The binding point is only valid for the pipelines stages specified.
///
/// The binding _must_ match with the corresponding shader interface.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DescriptorSetLayoutBinding {
    /// Descriptor bindings range.
    pub binding: DescriptorBinding,
    /// Type of the bound descriptors.
    pub ty: DescriptorType,
    /// Number of descriptors in the array.
    ///
    /// *Note*: If count is zero, the binding point is reserved
    /// and can't be accessed from any shader stages.
    pub count: DescriptorArrayIndex,
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


/// A descriptor pool is a collection of memory from which descriptor sets are allocated.
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
        layouts
            .into_iter()
            .map(|layout| self.allocate_set(layout.borrow()))
            .collect()
    }

    /// Resets a descriptor pool, releasing all resources from all the descriptor sets
    /// allocated from it and freeing the descriptor sets. Invalidates all descriptor
    /// sets allocated from the pool; trying to use one after the pool has been reset
    /// is undefined behavior.
    fn reset(&mut self);
}

/// DOC TODO
#[allow(missing_docs)]
pub struct DescriptorSetWrite<'a, B: Backend, W> {
    pub set: &'a B::DescriptorSet,
    /// *Note*: when there is more descriptors provided than
    /// array elements left in the specified binding starting
    /// at specified, offset, the updates are spilled onto
    /// the next binding (starting with offset 0), and so on.
    pub binding: DescriptorBinding,
    pub array_offset: DescriptorArrayIndex,
    pub descriptors: W,
}

/// DOC TODO
#[allow(missing_docs)]
#[derive(Clone)]
pub enum Descriptor<'a, B: Backend> {
    Sampler(&'a B::Sampler),
    Image(&'a B::ImageView, ImageLayout),
    CombinedImageSampler(&'a B::ImageView, ImageLayout, &'a B::Sampler),
    Buffer(&'a B::Buffer, Range<Option<Offset>>),
    TexelBuffer(&'a B::BufferView),
}


/// DOC TODO
#[allow(missing_docs)]
#[derive(Clone, Copy)]
pub struct DescriptorSetCopy<'a, B: Backend> {
    pub src_set: &'a B::DescriptorSet,
    pub src_binding: DescriptorBinding,
    pub src_array_offset: DescriptorArrayIndex,
    pub dst_set: &'a B::DescriptorSet,
    pub dst_binding: DescriptorBinding,
    pub dst_array_offset: DescriptorArrayIndex,
    pub count: usize,
}
