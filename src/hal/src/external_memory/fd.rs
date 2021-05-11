use super::{ExternalMemoryTypeFlags,ExternalMemoryType};

#[cfg(any(unix,doc))]
/// Unix file descriptor
#[derive(Debug)]
pub struct Fd(i32);
#[cfg(any(unix,doc))]
impl From<i32> for Fd {
    fn from(fd: i32)->Self {Self(fd)}
}
#[cfg(any(unix,doc))]
impl std::os::unix::io::AsRawFd for Fd {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {self.0}
}
#[cfg(any(unix,doc))]
impl std::ops::Deref for Fd {
    type Target = i32;
    fn deref(&self) -> &Self::Target {&self.0}
}



#[cfg(any(unix,doc))]
#[cfg_attr(feature = "unstable", doc(cfg(unix)))]
#[derive(Debug)]
#[allow(non_camel_case_types)]
/// External memory that rely on unix file descriptors
pub enum ExternalMemoryFd {
    /// Tmp
    OPAQUE_FD(Fd,u64),
    #[cfg(any(target_os = "linux",target_os = "android",doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(any(target_os = "linux",target_os = "android"))))]
    /// Tmp
    DMA_BUF(Fd,u64),
    #[cfg(any(target_os = "android",doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(target_os = "android")))]
    /// Tmp
    ANDROID_HARDWARE_BUFFER(Fd,u64),
}
impl ExternalMemoryFd {
    /// Get the fd
    pub fn get_fd(&self)->&Fd {
        match self {
            Self::OPAQUE_FD(fd,_)=>fd,
            #[cfg(any(target_os = "linux",target_os = "android",doc))]
            Self::DMA_BUF(fd,_)=>fd,
            #[cfg(any(target_os = "android",doc))]
            Self::ANDROID_HARDWARE_BUFFER(fd,_)=>fd,
        }
    }
    /// Get the size
    pub fn get_size(&self)->u64 {
        match self {
            Self::OPAQUE_FD(_,size)=>*size,
            #[cfg(any(target_os = "linux",target_os = "android",doc))]
            Self::DMA_BUF(_,size)=>*size,
            #[cfg(any(target_os = "android",doc))]
            Self::ANDROID_HARDWARE_BUFFER(_,size)=>*size,
        }
    }
    /// Get the external memory fd type
    pub fn get_external_memory_fd_type(&self)->ExternalMemoryFdType {
        match self {
            Self::OPAQUE_FD(_,_)=>ExternalMemoryFdType::OPAQUE_FD,
            #[cfg(any(target_os = "linux",target_os = "android",doc))]
            Self::DMA_BUF(_,_)=>ExternalMemoryFdType::DMA_BUF,
            #[cfg(any(target_os = "android",doc))]
            Self::ANDROID_HARDWARE_BUFFER(_,_)=>ExternalMemoryFdType::ANDROID_HARDWARE_BUFFER,
        }
    }
}

#[cfg(any(unix,doc))]
#[cfg_attr(feature = "unstable", doc(cfg(unix)))]
impl From<(ExternalMemoryFdType,Fd,u64)> for ExternalMemoryFd {
    fn from(tuple: (ExternalMemoryFdType,Fd,u64))->Self {
        match tuple.0 {
            ExternalMemoryFdType::OPAQUE_FD=>Self::OPAQUE_FD(tuple.1,tuple.2),
            #[cfg(any(target_os = "linux",target_os = "android",doc))]
            ExternalMemoryFdType::DMA_BUF=>Self::DMA_BUF(tuple.1,tuple.2),
            #[cfg(any(target_os = "android",doc))]
            ExternalMemoryFdType::ANDROID_HARDWARE_BUFFER=>Self::ANDROID_HARDWARE_BUFFER(tuple.1,tuple.2),
        }
    }
}

#[cfg(any(unix,doc))]
#[cfg_attr(feature = "unstable", doc(cfg(unix)))]
impl Into<(ExternalMemoryFdType,Fd,u64)> for ExternalMemoryFd {
    fn into(self)->(ExternalMemoryFdType,Fd,u64) {
        match self {
            Self::OPAQUE_FD(fd,size)=>(ExternalMemoryFdType::OPAQUE_FD,fd,size),
            #[cfg(any(target_os = "linux",target_os = "android",doc))]
            Self::DMA_BUF(fd,size)=>(ExternalMemoryFdType::OPAQUE_FD,fd,size),
            #[cfg(any(target_os = "android",doc))]
            Self::ANDROID_HARDWARE_BUFFER(fd,size)=>(ExternalMemoryFdType::OPAQUE_FD,fd,size),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(non_camel_case_types)]
#[cfg(any(unix,doc))]
#[cfg_attr(feature = "unstable", doc(cfg(unix)))]
/// Subgroup of ExternalMemoryType that export as file
pub enum ExternalMemoryFdType {
    /// Specifies a POSIX file descriptor handle that has only limited valid usage outside of Vulkan and other compatible APIs.
    /// It must be compatible with the POSIX system calls dup, dup2, close, and the non-standard system call dup3.
    /// Additionally, it must be transportable over a socket using an SCM_RIGHTS control message.
    /// It owns a reference to the underlying memory resource represented by its memory object.
    OPAQUE_FD,
    #[cfg(any(target_os = "linux",target_os = "android",doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(any(target_os = "linux",target_os = "android"))))]
    /// Is a file descriptor for a Linux dma_buf.
    /// It owns a reference to the underlying memory resource represented by its Vulkan memory object.
    DMA_BUF,
    #[cfg(any(target_os = "android",doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(target_os = "android")))]
    /// Specifies an AHardwareBuffer object defined by the Android NDK. See Android Hardware Buffers for more details of this handle type.
    ANDROID_HARDWARE_BUFFER,
}
impl From<ExternalMemoryFdType> for ExternalMemoryTypeFlags {
    fn from(external_memory_fd_type: ExternalMemoryFdType)->Self {
        match external_memory_fd_type {
            ExternalMemoryFdType::OPAQUE_FD=>Self::OPAQUE_FD,
            #[cfg(any(target_os = "linux",target_os = "android",doc))]
            ExternalMemoryFdType::DMA_BUF=>Self::DMA_BUF,
            #[cfg(any(target_os = "android",doc))]
            ExternalMemoryFdType::ANDROID_HARDWARE_BUFFER=>Self::ANDROID_HARDWARE_BUFFER,
        }
    }
}
impl From<ExternalMemoryFdType> for ExternalMemoryType {
    fn from(external_memory_fd_type: ExternalMemoryFdType)->Self {
        Self::Fd(external_memory_fd_type)
    }
}
