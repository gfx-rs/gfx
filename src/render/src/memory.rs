pub use core::memory::{Pod, cast_slice};

use std::marker::PhantomData;
use std::{ops, cmp, fmt, hash};
use std::sync::Arc;
use std::cell::UnsafeCell;

use {buffer, image};
use {Backend, Device};

/// How this memory will be used regarding GPU-CPU data flow.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum Usage {
    /// Full speed GPU access.
    /// Optimal for render targets and resourced memory.
    Data,
    /// CPU to GPU data flow with mapping.
    /// Used for staging for upload to GPU.
    Upload,
    /// GPU to CPU data flow with mapping.
    /// Used for staging for download from GPU.
    Download,
    // TODO: Hybrid,
}

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
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub flags Bind: u8 {
        /// Can be rendered into.
        const RENDER_TARGET    = 0x1,
        /// Can serve as a depth/stencil target.
        const DEPTH_STENCIL    = 0x2,
        /// Can be bound to the shader for reading.
        const SHADER_RESOURCE  = 0x4,
        /// Can be bound to the shader for writing.
        const UNORDERED_ACCESS = 0x8,
        /// Can be transfered from.
        const TRANSFER_SRC     = 0x10,
        /// Can be transfered into.
        const TRANSFER_DST     = 0x20,
    }
);

impl Bind {
    /// Is this memory bound to be a target ?
    pub fn is_target(&self) -> bool {
        self.intersects(RENDER_TARGET | DEPTH_STENCIL)
    }

    /// Is this memory bound to be mutated ?
    pub fn is_mutable(&self) -> bool {
        let mutable = TRANSFER_DST | UNORDERED_ACCESS | RENDER_TARGET | DEPTH_STENCIL;
        self.intersects(mutable)
    }
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
    pub flags Access: u8 {
        /// Read access
        const READ  = 0x1,
        /// Write access
        const WRITE = 0x2,
        /// Full access
        const RW    = 0x3,
    }
);

pub type ReleaseFn = Box<FnMut()>; // TODO?: FnOnce
pub struct Memory {
    release: ReleaseFn,
    pub usage: Usage,
}

impl Memory {
    pub fn new(release: ReleaseFn, usage: Usage) -> Self {
        Memory { release, usage }
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        (self.release)();
    }
}

impl fmt::Debug for Memory {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Memory({:?})", self.usage)
    }
}

// TODO: errors
pub trait Allocator<B: Backend> {
    fn allocate_buffer(&mut self,
        device: &mut Device<B>,
        usage: &buffer::Usage,
        buffer: B::UnboundBuffer
    ) -> (B::Buffer, Memory);
    
    fn allocate_image(&mut self,
        device: &mut Device<B>,
        usage: &image::Usage,
        image: B::UnboundImage
    ) -> (B::Image, Memory);
}

#[derive(Debug)]
pub struct Typed<I, T> {
    inner: I,
    phantom: PhantomData<T>,
}

impl<I, T> Typed<I, T> {
    pub fn new(inner: I) -> Self {
        Typed {
            inner,
            phantom: PhantomData,
        }
    }
}

impl<I: Clone, T> Clone for Typed<I, T> {
    fn clone(&self) -> Self {
        Self::new(self.inner.clone())
    }
}

impl<I, T> cmp::PartialEq for Typed<I, T>
    where I: cmp::PartialEq
{
    fn eq(&self, other: &Typed<I, T>) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl<I, T> cmp::Eq for Typed<I, T>
    where I: cmp::Eq
{}

impl<I, T> hash::Hash for Typed<I, T>
    where I: hash::Hash
{
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<I, T> ops::Deref for Typed<I, T> {
    type Target = I;

    fn deref(&self) -> &I {
        &self.inner
    }
}

impl<I, T> ops::DerefMut for Typed<I, T> {
    fn deref_mut(&mut self) -> &mut I {
        &mut self.inner
    }
}

/// This is the unique owner of the inner struct.
#[derive(Debug)]
pub struct DropDelayed<T>(Arc<UnsafeCell<T>>);
/// Keep-alive without any access (only Drop if last one).
pub struct DropDelayer<T>(Arc<UnsafeCell<T>>);

impl<T> DropDelayed<T> {
    pub fn new(inner: T) -> Self {
        DropDelayed(Arc::new(UnsafeCell::new(inner)))
    }

    pub fn drop_delayer(&self) -> DropDelayer<T> {
        DropDelayer(self.0.clone())
    }
}

impl<T> ops::Deref for DropDelayed<T> {
    type Target = T;
    fn deref(&self) -> &T { unsafe { &*self.0.get() } }
}

impl<T> ops::DerefMut for DropDelayed<T> {
    fn deref_mut(&mut self) -> &mut T { unsafe { &mut *self.0.get() } }
}

impl<T> Clone for DropDelayer<T> {
    fn clone(&self) -> Self {
        DropDelayer(self.0.clone())
    }
}

impl<T> fmt::Debug for DropDelayer<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "DropDelayer")
    }
}
