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

pub enum PlatformMemory {
    #[cfg(any(unix, doc))]
    Fd(Fd),
    #[cfg(any(windows, doc))]
    Handle(Handle),
    Ptr(Ptr),
}

#[cfg(unix)]
impl From<Fd> for PlatformMemory {
    fn from(fd: Fd) -> Self {
        Self::Fd(fd)
    }
}

#[cfg(windows)]
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

pub enum PlatformMemoryType {
    #[cfg(any(unix, doc))]
    Fd,
    #[cfg(any(windows, doc))]
    Handle,
    Ptr,
}
