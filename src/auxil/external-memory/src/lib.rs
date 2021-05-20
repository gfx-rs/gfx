#[cfg(any(unix))]
#[cfg_attr(feature = "unstable", doc(cfg(unix)))]
mod fd;
#[cfg(any(unix))]
#[cfg_attr(feature = "unstable", doc(cfg(unix)))]
pub use fd::*;

#[cfg(any(windows))]
#[cfg_attr(feature = "unstable", doc(cfg(windows)))]
mod handle;
#[cfg(any(windows))]
#[cfg_attr(feature = "unstable", doc(cfg(windows)))]
pub use handle::*;

mod ptr;
pub use ptr::*;

pub enum PlatformMemory {
    #[cfg(any(unix))]
    #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
    Fd(Fd),
    #[cfg(any(windows))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    Handle(Handle),
    Ptr(Ptr),
}

#[cfg(any(unix))]
#[cfg_attr(feature = "unstable", doc(cfg(unix)))]
impl From<Fd> for PlatformMemory {
    fn from(fd: Fd) -> Self {
        Self::Fd(fd)
    }
}

#[cfg(any(windows))]
#[cfg_attr(feature = "unstable", doc(cfg(windows)))]
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
    #[cfg(any(unix))]
    #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
    Fd,
    #[cfg(any(windows))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    Handle,
    Ptr,
}
