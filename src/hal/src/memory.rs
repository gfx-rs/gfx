//! Types to describe the properties of memory allocated for gfx resources.

use std::mem;
use std::ops::Range;
use {buffer, image};
use Backend;

/// A trait for plain-old-data types.
///
/// A POD type does not have invalid bit patterns and can be safely
/// created from arbitrary bit pattern.
/// The `Pod` trait is implemented for standard integer and floating point numbers as well as
/// common arrays of them (for example `[f32; 2]`).
pub unsafe trait Pod: Copy {}

macro_rules! impl_pod {
    ( ty = $($ty:ty)* ) => { $( unsafe impl Pod for $ty {} )* };
    ( ar = $($tt:expr)* ) => { $( unsafe impl<T: Pod> Pod for [T; $tt] {} )* };
}

impl_pod! { ty = isize usize i8 u8 i16 u16 i32 u32 i64 u64 f32 f64 }
impl_pod! { ar =
    0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32
}

unsafe impl<T: Pod, U: Pod> Pod for (T, U) {}

/// Cast a slice from one POD type to another.
pub fn cast_slice<A: Pod, B: Pod>(slice: &[A]) -> &[B] {
    use std::slice;

    let raw_len = mem::size_of::<A>().wrapping_mul(slice.len());
    let len = raw_len / mem::size_of::<B>();
    assert_eq!(raw_len, mem::size_of::<B>().wrapping_mul(len));
    unsafe {
        slice::from_raw_parts(slice.as_ptr() as *const B, len)
    }
}

bitflags!(
    /// Memory property flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Properties: u16 {
        /// Device local memory on the GPU.
        const DEVICE_LOCAL   = 0x1;

        /// CPU-GPU coherent.
        ///
        /// Non-coherent memory requires explicit flushing.
        const COHERENT     = 0x2;

        /// Host visible memory can be accessed by the CPU.
        ///
        /// Backends must provide at least one cpu visible memory.
        const CPU_VISIBLE   = 0x4;

        /// Cached memory by the CPU
        const CPU_CACHED = 0x8;

        /// Memory that may be lazily allocated as needed on the GPU
        /// and *must not* be visible to the CPU.
        const LAZILY_ALLOCATED = 0x20;
    }
);

bitflags!(
    /// Barrier dependency flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct Dependencies: u32 {
        /// Specifies the memory dependency to be framebuffer-local.
        const BY_REGION    = 0x1;
        //const VIEW_LOCAL   = 0x2;
        //const DEVICE_GROUP = 0x4;
    }
);

// DOC TODO: Could be better, but I don't know how to do this without 
// trying to explain the whole synchronization model.
/// A [memory barrier](https://www.khronos.org/registry/vulkan/specs/1.0/html/vkspec.html#synchronization-memory-barriers)
/// type for either buffers or images.
#[allow(missing_docs)] 
#[derive(Clone, Debug)]
pub enum Barrier<'a, B: Backend> {
    /// Applies the given access flags to all buffers in the range.
    AllBuffers(Range<buffer::Access>),
    /// Applies the given access flags to all images in the range.
    AllImages(Range<image::Access>),
    /// A memory barrier that defines access to a buffer.
    Buffer {
        /// The access flags controlling the buffer.
        states: Range<buffer::State>,
        /// The buffer the barrier controls.
        target: &'a B::Buffer,
    },
    /// A memory barrier that defines access to (a subset of) an image.
    Image {
        /// The access flags controlling the image.
        states: Range<image::State>,
        /// The image the barrier controls.
        target: &'a B::Image,
        /// A `SubresourceRange` that defines which section of an image the barrier applies to.
        range: image::SubresourceRange,
    },
}

/// Memory requirements for a certain resource (buffer/image).
#[derive(Clone, Copy, Debug)]
pub struct Requirements {
    /// Size in the memory.
    pub size: u64,
    /// Memory alignment.
    pub alignment: u64,
    /// Supported memory types.
    pub type_mask: u64,
}
