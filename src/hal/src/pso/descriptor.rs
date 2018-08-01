//! Descriptor sets and layouts.
//! 
//! A [`Descriptor`] is an object that describes the connection between a resource, such as
//! an `Image` or `Buffer`, and a variable in a shader. Descriptors are organized into
//! `DescriptorSet`s, each of which contains multiple descriptors that are bound and unbound to
//! shaders as a single unit. The contents of each descriptor in a set is defined by a
//! `DescriptorSetLayout` which is in turn built of [`DescriptorSetLayoutBinding`]s. A `DescriptorSet`
//! is then allocated from a [`DescriptorPool`] using the `DescriptorSetLayout`, and specific [`Descriptor`]s are
//! then bound to each binding point in the set using a [`DescriptorSetWrite`] and/or [`DescriptorSetCopy`]. 
//! Each descriptor set may contain descriptors to multiple different sorts of resources, and a shader may 
//! use multiple descriptor sets at a time.
//! 
//! [`Descriptor`]: enum.Descriptor.html
//! [`DescriptorSetLayoutBinding`]: struct.DescriptorSetLayoutBinding.html
//! [`DescriptorPool`]: trait.DescriptorPool.html
//! [`DescriptorSetWrite`]: struct.DescriptorSetWrite.html
//! [`DescriptorSetCopy`]: struct.DescriptorSetWrite.html

use std::borrow::Borrow;
use std::fmt;
use std::ops::Range;

use {Backend};
use buffer::Offset;
use image::Layout;
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
    /// A uniform buffer that can be bound with an offset into its memory with minimal performance impact,
    /// usually used to store pieces of "uniform" data that change per draw call rather than
    /// per render pass.
    UniformBufferDynamic = 8,
    ///
    StorageBufferDynamic = 9,
    /// Allows unfiltered loads of pixel local data in the fragment shader.
    InputAttachment = 10,
}

/// Information about the contents of and in which stages descriptors may be bound to a descriptor
/// set at a certain binding point. Multiple `DescriptorSetLayoutBinding`s are assembled into
/// a `DescriptorSetLayout`, which is then allocated into a `DescriptorSet` using a
/// [`DescriptorPool`].
///
/// A descriptor set consists of multiple binding points.
/// Each binding point contains one or multiple descriptors of a certain type.
/// The binding point is only valid for the pipelines stages specified.
///
/// The binding _must_ match with the corresponding shader interface.
/// 
/// [`DescriptorPool`]: trait.DescriptorPool.html
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
    /// Use the associated list of immutable samplers.
    pub immutable_samplers: bool,
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

/// An error allocating descriptor sets from a pool.
#[derive(Fail, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocationError {
    /// Memory allocation on the host side failed.
    /// This could be caused by a lack of memory or pool fragmentation.
    #[fail(display = "Host memory allocation failed.")]
    OutOfHostMemory,
    /// Memory allocation on the host side failed.
    /// This could be caused by a lack of memory or pool fragmentation.
    #[fail(display = "Device memory allocation failed.")]
    OutOfDeviceMemory,
    /// Memory allocation failed as there is not enough in the pool.
    /// This could be caused by too many descriptor sets being created.
    #[fail(display = "Descriptor pool memory allocation failed.")]
    OutOfPoolMemory,
    /// Memory allocation failed due to pool fragmentation.
    #[fail(display = "Descriptor pool is fragmented.")]
    FragmentedPool,
    /// Descriptor set allocation failed as the layout is incompatible with the pool.
    #[fail(display = "Descriptor layout incompatible with pool.")]
    IncompatibleLayout,
}

/// A descriptor pool is a collection of memory from which descriptor sets are allocated.
pub trait DescriptorPool<B: Backend>: Send + Sync + fmt::Debug {
    /// Allocate a descriptor set from the pool.
    ///
    /// The descriptor set will be allocated from the pool according to the corresponding set layout. However,
    /// specific descriptors must still be written to the set before use using a [`DescriptorSetWrite`] or
    /// [`DescriptorSetCopy`].
    /// 
    /// Descriptors will become invalid once the pool is reset. Usage of invalidated descriptor sets results
    /// in undefined behavior.
    /// 
    /// [`DescriptorSetWrite`]: struct.DescriptorSetWrite.html
    /// [`DescriptorSetCopy`]: struct.DescriptorSetCopy.html
    fn allocate_set(&mut self, layout: &B::DescriptorSetLayout) -> Result<B::DescriptorSet, AllocationError> {
        let mut sets = Vec::with_capacity(1);
        self.allocate_sets(Some(layout), &mut sets)
            .map(|_| sets.remove(0))
    }

    /// Allocate one or multiple descriptor sets from the pool.
    ///
    /// The descriptor set will be allocated from the pool according to the corresponding set layout. However,
    /// specific descriptors must still be written to the set before use using a [`DescriptorSetWrite`] or
    /// [`DescriptorSetCopy`].
    /// 
    /// Each descriptor set will be allocated from the pool according to the corresponding set layout.
    /// Descriptors will become invalid once the pool is reset. Usage of invalidated descriptor sets results
    /// in undefined behavior.
    /// 
    /// [`DescriptorSetWrite`]: struct.DescriptorSetWrite.html
    /// [`DescriptorSetCopy`]: struct.DescriptorSetCopy.html
    fn allocate_sets<I>(&mut self, layouts: I, sets: &mut Vec<B::DescriptorSet>) -> Result<(), AllocationError>
    where
        I: IntoIterator,
        I::Item: Borrow<B::DescriptorSetLayout>,
    {
        let base = sets.len();
        for layout in layouts {
            match self.allocate_set(layout.borrow()) {
                Ok(set) => sets.push(set),
                Err(e) => {
                    self.free_sets(sets.drain(base ..));
                    return Err(e)
                }
            }
        }
        Ok(())
    }

    /// Free the given descriptor sets provided as an iterator.
    fn free_sets<I>(&mut self, descriptor_sets: I)
    where
        I: IntoIterator<Item = B::DescriptorSet>;

    /// Resets a descriptor pool, releasing all resources from all the descriptor sets
    /// allocated from it and freeing the descriptor sets. Invalidates all descriptor
    /// sets allocated from the pool; trying to use one after the pool has been reset
    /// is undefined behavior.
    fn reset(&mut self);
}

/// Writes the actual descriptors to be bound into a descriptor set. Should be provided
/// to the `write_descriptor_sets` method of a `Device`.
#[allow(missing_docs)]
pub struct DescriptorSetWrite<'a, B: Backend, WI>
    where WI: IntoIterator,
          WI::Item: Borrow<Descriptor<'a, B>>
{
    pub set: &'a B::DescriptorSet,
    /// *Note*: when there is more descriptors provided than
    /// array elements left in the specified binding starting
    /// at specified, offset, the updates are spilled onto
    /// the next binding (starting with offset 0), and so on.
    pub binding: DescriptorBinding,
    pub array_offset: DescriptorArrayIndex,
    pub descriptors: WI,
}

/// A handle to a specific shader resource that can be bound for use in a `DescriptorSet`.
/// Usually provided in a [`DescriptorSetWrite`]
/// 
/// [`DescriptorSetWrite`]: struct.DescriptorSetWrite.html
#[allow(missing_docs)]
#[derive(Clone)]
pub enum Descriptor<'a, B: Backend> {
    Sampler(&'a B::Sampler),
    Image(&'a B::ImageView, Layout),
    CombinedImageSampler(&'a B::ImageView, Layout, &'a B::Sampler),
    Buffer(&'a B::Buffer, Range<Option<Offset>>),
    UniformTexelBuffer(&'a B::BufferView),
    StorageTexelBuffer(&'a B::BufferView),
}


/// Copies a range of descriptors to be bound from one descriptor set to another Should be
/// provided to the `copy_descriptor_sets` method of a `Device`. 
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
