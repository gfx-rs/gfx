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
impl PlatformMemory {
    #[cfg(any(unix, doc))]
    pub fn fd(&self)->Option<&Fd> {
        match self {
            Self::Fd(fd)=>Some(fd),
            #[cfg(windows)]
            Self::Handle(_)=>None,
            Self::Ptr(_)=>None,
        }
    }
    #[cfg(any(windows, doc))]
    pub fn handle(&self)->Option<&Handle> {
        match self {
            Self::Handle(handle)=>Some(handle),
            #[cfg(unix)]
            Self::Fd(_)=>None,
            Self::Ptr(_)=>None,
        }
    }
    pub fn ptr(&self)->Option<&Ptr> {
        match self {
            Self::Ptr(ptr)=>Some(ptr),
            #[cfg(unix)]
            Self::Fd(_)=>None,
            #[cfg(windows)]
            Self::Handle(_)=>None
        }
    }
}

#[cfg(any(unix, doc))]
impl std::convert::TryInto<Fd> for PlatformMemory{
    type Error = &'static str;
    fn try_into(self) -> Result<Fd, Self::Error>{
        match self {
            Self::Fd(fd)=>Ok(fd),
            #[cfg(windows)]
            Self::Handle(_)=>Err("PlatformMemory does not contain an fd"),
            Self::Ptr(_)=>Err("PlatformMemory does not contain an fd"),
        }
    }
}

#[cfg(any(windows, doc))]
impl std::convert::TryInto<Handle> for PlatformMemory{
    type Error = &'static str;
    fn try_into(self) -> Result<Handle, Self::Error>{
        match self {
            Self::Handle(handle)=>Ok(handle),
            #[cfg(unix)]
            Self::Fd(_)=>Err("PlatformMemory does not contain an handle"),
            Self::Ptr(_)=>Err("PlatformMemory does not contain an handle"),
        }
    }
}


impl std::convert::TryInto<Ptr> for PlatformMemory{
    type Error = &'static str;
    fn try_into(self) -> Result<Ptr, Self::Error>{
        match self {
            Self::Ptr(ptr)=>Ok(ptr),
            #[cfg(unix)]
            Self::Fd(_)=>Err("PlatformMemory does not contain a ptr"),
            #[cfg(windows)]
            Self::Handle(_)=>Err("PlatformMemory does not contain a ptr")
        }
    }
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
