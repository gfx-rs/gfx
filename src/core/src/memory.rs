// Copyright 2016 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Types to describe the properties of memory allocated for gfx resources.

use std::mem;

// TODO: It would be useful to document what parameters these map to in D3D, vulkan, etc.

/// How this memory will be used regarding GPU-CPU data flow.
///
/// This information is used to create resources
/// (see [gfx::Factory](../trait.Factory.html#overview)).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum Usage {
    /// Full speed GPU access.
    /// Optimal for render targets and resourced memory.
    Data,
    /// CPU to GPU data flow with update commands.
    /// Used for dynamic buffer data, typically constant buffers.
    Dynamic,
    /// CPU to GPU data flow with mapping.
    /// Used for staging for upload to GPU.
    Upload,
    /// GPU to CPU data flow with mapping.
    /// Used for staging for download from GPU.
    Download,
}

bitflags!(
    /// Flags providing information about the type of memory access to a resource.
    ///
    /// An `Access` value can be a combination of the the following bit patterns:
    ///
    /// - [`READ`](constant.READ.html)
    /// - [`WRITE`](constant.WRITE.html)
    /// - Or [`RW`](constant.RW.html) which is equivalent to `READ` and `WRITE`.
    ///
    /// This information is used to create resources
    /// (see [gfx::Factory](trait.Factory.html#overview)).
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Access: u8 {
        /// Read access
        const READ  = 0x1;
        /// Write access
        const WRITE = 0x2;
        /// Full access
        const RW    = 0x3;
    }
);

bitflags!(
    /// Flags providing information about the usage of a resource.
    ///
    /// A `Bind` value can be a combination of the following bit patterns:
    ///
    /// - [`RENDER_TARGET`](constant.RENDER_TARGET.html)
    /// - [`DEPTH_STENCIL`](constant.DEPTH_STENCIL.html)
    /// - [`SHADER_RESOURCE`](constant.SHADER_RESOURCE.html)
    /// - [`UNORDERED_ACCESS`](constant.UNORDERED_ACCESS.html)
    /// - [`TRANSFER_SRC`](constant.TRANSFER_SRC.html)
    /// - [`TRANSFER_DST`](constant.TRANSFER_DST.html)
    ///
    ///
    /// This information is used to create resources
    /// (see [gfx::Factory](trait.Factory.html#overview)).
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Bind: u8 {
        /// Can be rendered into.
        const RENDER_TARGET    = 0x1;
        /// Can serve as a depth/stencil target.
        const DEPTH_STENCIL    = 0x2;
        /// Can be bound to the shader for reading.
        const SHADER_RESOURCE  = 0x4;
        /// Can be bound to the shader for writing.
        const UNORDERED_ACCESS = 0x8;
        /// Can be transfered from.
        const TRANSFER_SRC     = 0x10;
        /// Can be transfered into.
        const TRANSFER_DST     = 0x20;
    }
);

impl Bind {
    /// Is this memory bound to be mutated ?
    pub fn is_mutable(&self) -> bool {
        let mutable = Self::TRANSFER_DST | Self::UNORDERED_ACCESS |
                      Self::RENDER_TARGET | Self::DEPTH_STENCIL;
        self.intersects(mutable)
    }
}

/// A service trait used to get the raw data out of strong types.
/// Not meant for public use.
#[doc(hidden)]
pub trait Typed: Sized {
    /// The raw type behind the phantom.
    type Raw;
    /// Crete a new phantom from the raw type.
    fn new(raw: Self::Raw) -> Self;
    /// Get an internal reference to the raw type.
    fn raw(&self) -> &Self::Raw;
}

/// A trait for plain-old-data types.
///
/// A POD type does not have invalid bit patterns and can be safely
/// created from arbitrary bit pattern.
/// The `Pod` trait is implemented for standard integer and floating point numbers as well as
/// common arrays of them (for example `[f32; 2]`).
pub unsafe trait Pod {}

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
