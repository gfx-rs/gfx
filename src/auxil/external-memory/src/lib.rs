#[cfg(any(unix, doc))]
mod fd;
#[cfg(any(unix, doc))]
pub use fd::*;

#[cfg(any(windows, doc))]
mod handle;
#[cfg(any(windows, doc))]
pub use handle::*;

mod ptr;
pub use ptr::*;

/// Representation of an os specific memory.
pub enum PlatformMemory {
    #[cfg(any(unix, doc))]
    /// Unix file descriptor.
    Fd(Fd),
    #[cfg(any(windows, doc))]
    /// Windows handle.
    Handle(Handle),
    /// Host pointer.
    Ptr(Ptr),
}

#[cfg(any(unix,docs))]
impl From<Fd> for PlatformMemory {
    fn from(fd: Fd) -> Self {
        Self::Fd(fd)
    }
}

#[cfg(any(windows,doc))]
impl From<Handle> for PlatformMemory {
    fn from(handle: Handle) -> Self {
        Self::Handle(handle)
    }
}

impl From<Ptr> for PlatformMemory {
    fn from(ptr: Ptr) -> Self {
        Self::Ptr(ptr)
    }
}

/// Representation of os specific memory types.
pub enum PlatformMemoryType {
    #[cfg(any(unix, doc))]
    Fd,
    #[cfg(any(windows, doc))]
    Handle,
    Ptr,
}
