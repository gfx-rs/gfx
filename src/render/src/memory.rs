pub use core::memory::{Pod, cast_slice};

use std::marker::PhantomData;
use std::{ops, cmp, fmt, hash};
//use std::convert::AsRef;
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
        usage: buffer::Usage,
        buffer: B::UnboundBuffer
    ) -> (B::Buffer, Memory);
    
    fn allocate_image(&mut self,
        device: &mut Device<B>,
        usage: image::Usage,
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

impl<I, T> AsRef<I> for Typed<I, T> {
    fn as_ref(&self) -> &I {
        &self.inner
    }
}

/// This is the unique owner of the inner struct.
#[derive(Debug)]
pub struct Provider<T>(Arc<UnsafeCell<T>>);
/// Keep-alive without any access (only Drop if last one).
pub struct Dependency<T>(Arc<UnsafeCell<T>>);

impl<T> Provider<T> {
    pub fn new(inner: T) -> Self {
        Provider(Arc::new(UnsafeCell::new(inner)))
    }

    pub fn dependency(&self) -> Dependency<T> {
        Dependency(self.0.clone())
    }
}

impl<T> ops::Deref for Provider<T> {
    type Target = T;
    fn deref(&self) -> &T { unsafe { &*self.0.get() } }
}

impl<T> ops::DerefMut for Provider<T> {
    fn deref_mut(&mut self) -> &mut T { unsafe { &mut *self.0.get() } }
}

impl<T> Clone for Dependency<T> {
    fn clone(&self) -> Self {
        Dependency(self.0.clone())
    }
}

impl<T> fmt::Debug for Dependency<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Dependency")
    }
}
