//! Structures related to the import external memory functionality

use crate::device::{AllocationError,OutOfMemory};
use crate::buffer::CreationError;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External buffer create error
pub enum ExternalBufferCreateError {
    /// Creation error.
    #[error(transparent)]
    CreationError(#[from] CreationError),

    /// Invalid external handle.
    #[error("The used external handle or the combination of them is invalid")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}
impl From<OutOfMemory> for ExternalBufferCreateError {
    fn from(error: OutOfMemory)->Self {Self::CreationError(error.into())}
}

/*
impl From<CreationError> for ExternalBufferCreateError {
    fn from(error: CreationError)->Self {Self::CreationError(error.into())}
}
*/
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory import error
pub enum ExternalMemoryImportError {
    /// Allocation error.
    #[error(transparent)]
    AllocationError(#[from] AllocationError),

    /// Creation error.
    #[error(transparent)]
    CreationError(#[from] CreationError),

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,

    /// Unsupported parameters.
    #[error("Unsupported parameters")]
    UnsupportedParameters,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}

impl From<OutOfMemory> for ExternalMemoryImportError {
    fn from(error: OutOfMemory)->Self {Self::AllocationError(error.into())}
}
/*
impl From<CreationError> for ExternalMemoryImportError {
    fn from(error: CreationError)->Self {Self::CreationError(error.into())}
}
*/
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory export error
pub enum ExternalMemoryExportError {
    /// Too many objects.
    #[error("Too many objects")]
    TooManyObjects,

    /// Out of host memory.
    #[error("Out of host memory")]
    OutOfHostMemory,

    /// Unsupported parameters.
    #[error("Unsupported parameters")]
    UnsupportedParameters,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}


#[cfg(unix)]
/// Unix file descriptor
#[derive(Debug)]
pub struct Fd(i32);
#[cfg(unix)]
impl From<i32> for Fd {
    fn from(fd: i32)->Self {Self(fd)}
}
impl std::ops::Deref for Fd {
    type Target = i32;
    fn deref(&self) -> &Self::Target {&self.0}
}
#[cfg(windows)]
/// Windows handle
#[derive(Debug)]
pub struct Handle(*mut std::ffi::c_void);
#[cfg(windows)]
impl From<*mut std::ffi::c_void> for Handle {
    fn from(ptr: *mut std::ffi::c_void)->Self {Self(ptr)}
}
#[cfg(windows)]
impl std::ops::Deref for Handle {
    type Target = *mut std::ffi::c_void;
    fn deref(&self) -> &Self::Target {&self.0}
}

/// Pointer to a host allocated memory
#[derive(Debug)]
pub struct Ptr(*mut std::ffi::c_void);
impl From<*mut std::ffi::c_void> for Ptr {
    fn from(ptr: *mut std::ffi::c_void)->Self {Self(ptr)}
}
impl std::ops::Deref for Ptr {
    type Target = *mut std::ffi::c_void;
    fn deref(&self) -> &Self::Target {&self.0}
}

/// Enumeration for all the external handles
#[derive(Debug)]
pub enum ExternalHandle {
    #[cfg(unix)]
    /// Unix file descriptor
    Fd(Fd),
    #[cfg(windows)]
    /// Window handle
    Handle(Handle),
    /// Pointer to a host allocated memory
    Ptr(Ptr)
}

#[cfg(unix)]
impl From<Fd> for ExternalHandle {
    fn from(fd: Fd)->Self {Self::Fd(fd)}
}
#[cfg(windows)]
impl From<Handle> for ExternalHandle {
    fn from(handle: Handle)->Self {Self::Handle(handle)}
}
impl From<Ptr> for ExternalHandle {
    fn from(ptr: Ptr)->Self {Self::Ptr(ptr)}
}

#[derive(Clone, Debug, PartialEq)]
/// External memory handle
pub enum ExternalMemoryHandle {
    #[cfg(unix)]
    /// Tmp
    OpaqueFd{
        /// File descriptor,
        fd: std::os::raw::c_int,
        /// File descriptor size
        size: u64,
    },
    #[cfg(windows)]
    /// Tmp
    OpaqueWin32{
        /// File descriptor,
        handle: *mut std::ffi::c_void,
        /// File descriptor size
        size: u64,
    },
    #[cfg(windows)]
    /// Tmp
    OpaqueWin32Kmt{
        /// Handle
        handle: *mut std::ffi::c_void,
        /// File descriptor size
        size: u64,
    },
    #[cfg(windows)]
    /// Tmp. Size is ignored.
    D3D11Texture{
        /// Handle
        handle: *mut std::ffi::c_void,
    },
    #[cfg(windows)]
    /// Tmp. Size is ignored
    D3D11TextureKmt{
        /// Handle
        handle: *mut std::ffi::c_void,
    },
    #[cfg(windows)]
    /// Tmp
    D3D12Heap{
        /// Handle
        handle: *mut std::ffi::c_void,
        /// File descriptor size
        size: u64,
    },
    #[cfg(windows)]
    /// Tmp
    D3D12Resource{
        /// Handle
        handle: *mut std::ffi::c_void,
    },
    #[cfg(any(target_os = "linux",target_os = "android"))]
    /// Tmp
    DmaBuf{
        /// File descriptor,
        fd: std::os::raw::c_int,
        /// File descriptor size
        size: u64,
    },
    #[cfg(target_os = "android")]
    /// Tmp
    AndroidHardwareBuffer{
        /// File descriptor,
        fd: std::os::raw::c_int,
        /// File descriptor size
        size: u64,
    },
    /// Tmp
    HostAllocation{
        /// Pointer
        ptr: *mut std::ffi::c_void,
        /// File descriptor size
        size: u64
    },
    /// Tmp
    HostMappedForeignMemory{
        /// Pointer
        ptr: *mut std::ffi::c_void,
        /// File descriptor size
        size: u64
    },
}
impl ExternalMemoryHandle {
    /// Get the external memory type from this handle
    pub fn as_type(&self)->ExternalMemoryType {
        match self{
            #[cfg(unix)]
            Self::OpaqueFd{fd: _,size: _}=>ExternalMemoryType::OpaqueFd,
            #[cfg(windows)]
            Self::OpaqueWin32{handle: _,size: _}=>ExternalMemoryType::OpaqueWin32,
            #[cfg(windows)]
            Self::OpaqueWin32Kmt{handle: _,size: _}=>ExternalMemoryType::OpaqueWin32Kmt,
            #[cfg(windows)]
            Self::D3D11Texture{handle: _}=>ExternalMemoryType::D3D11Texture,
            #[cfg(windows)]
            Self::D3D11TextureKmt{handle: _}=>ExternalMemoryType::D3D11TextureKmt,
            #[cfg(windows)]
            Self::D3D12Heap{handle: _,size: _}=>ExternalMemoryType::D3D12Heap,
            #[cfg(windows)]
            Self::D3D12Resource{handle: _}=>ExternalMemoryType::D3D12Resource,
            #[cfg(any(target_os = "linux",target_os = "android"))]
            Self::DmaBuf{fd: _,size: _}=>ExternalMemoryType::DmaBuf,
            #[cfg(target_os = "android")]
            Self::AndroidHardwareBuffer{fd: _,size: _}=>ExternalMemoryType::AndroidHardwareBuffer,
            Self::HostAllocation{ptr: _,size: _}=>ExternalMemoryType::HostAllocation,
            Self::HostMappedForeignMemory{ptr: _,size: _}=>ExternalMemoryType::HostMappedForeignMemory,
        }
    }
    /// Extract the info in a tuple format
    pub fn extract(self)->(ExternalHandle,u64,ExternalMemoryType) {
        match self {
            #[cfg(unix)]
            ExternalMemoryHandle::OpaqueFd{fd,size}=>(Fd::from(fd).into(),size,ExternalMemoryType::OpaqueFd),
            #[cfg(windows)]
            ExternalMemoryHandle::OpaqueWin32{handle,size}=>(Handle::from(handle).into(),size,ExternalMemoryType::OpaqueWin32),
            #[cfg(windows)]
            ExternalMemoryHandle::OpaqueWin32Kmt{handle,size}=>(Handle::from(handle).into(),size,ExternalMemoryType::OpaqueWin32Kmt),
            #[cfg(windows)]
            ExternalMemoryHandle::D3D11Texture{handle}=>(Handle::from(handle).into(),0,ExternalMemoryType::D3D11Texture),
            #[cfg(windows)]
            ExternalMemoryHandle::D3D11TextureKmt{handle}=>(Handle::from(handle).into(),0,ExternalMemoryType::D3D11TextureKmt),
            #[cfg(windows)]
            ExternalMemoryHandle::D3D12Heap{handle,size}=>(Handle::from(handle).into(),size,ExternalMemoryType::D3D12Heap),
            #[cfg(windows)]
            ExternalMemoryHandle::D3D12Resource{handle}=>(Handle::from(handle).into(),0,ExternalMemoryType::D3D12Resource),
            #[cfg(any(target_os = "linux",target_os = "android"))]
            ExternalMemoryHandle::DmaBuf{fd,size}=>(Fd::from(fd).into(),size,ExternalMemoryType::DmaBuf),
            #[cfg(target_os = "android")]
            ExternalMemoryHandle::AndroidHardwareBuffer{fd,size}=>(Fd::from(fd).into(),size,ExternalMemoryType::AndroidHardwareBuffer),
            ExternalMemoryHandle::HostAllocation{ptr,size}=>(Ptr::from(ptr).into(),size,ExternalMemoryType::HostAllocation),
            ExternalMemoryHandle::HostMappedForeignMemory{ptr,size}=>(Ptr::from(ptr).into(),size,ExternalMemoryType::HostMappedForeignMemory),
        }
    }
}

/// External memory types
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExternalMemoryType {
    #[cfg(unix)]
    /// Specifies a POSIX file descriptor handle that has only limited valid usage outside of Vulkan and other compatible APIs.
    /// It must be compatible with the POSIX system calls dup, dup2, close, and the non-standard system call dup3.
    /// Additionally, it must be transportable over a socket using an SCM_RIGHTS control message.
    /// It owns a reference to the underlying memory resource represented by its memory object.
    OpaqueFd,
    #[cfg(windows)]
    /// Specifies an NT handle that has only limited valid usage outside of Vulkan and other compatible APIs.
    /// It must be compatible with the functions DuplicateHandle, CloseHandle, CompareObjectHandles, GetHandleInformation, and SetHandleInformation.
    /// It owns a reference to the underlying memory resource represented by its memory object.
    OpaqueWin32,
    #[cfg(windows)]
    /// Specifies a global share handle that has only limited valid usage outside of Vulkan and other compatible APIs.
    /// It is not compatible with any native APIs.
    /// It does not own a reference to the underlying memory resource represented by its memory object, and will therefore become invalid when all the memory objects with it are destroyed.
    OpaqueWin32Kmt,
    #[cfg(windows)]
    /// Specifies an NT handle returned by IDXGIResource1::CreateSharedHandle referring to a Direct3D 10 or 11 texture resource.
    /// It owns a reference to the memory used by the Direct3D resource.
    D3D11Texture,
    #[cfg(windows)]
    /// Specifies a global share handle returned by IDXGIResource::GetSharedHandle referring to a Direct3D 10 or 11 texture resource.
    /// It does not own a reference to the underlying Direct3D resource, and will therefore become invalid when all the memory objects and Direct3D resources associated with it are destroyed.
    D3D11TextureKmt,
    #[cfg(windows)]
    /// Specifies an NT handle returned by ID3D12Device::CreateSharedHandle referring to a Direct3D 12 heap resource.
    /// It owns a reference to the resources used by the Direct3D heap.
    D3D12Heap,
    #[cfg(windows)]
    /// Specifies an NT handle returned by ID3D12Device::CreateSharedHandle referring to a Direct3D 12 committed resource.
    /// It owns a reference to the memory used by the Direct3D resource.
    D3D12Resource,
    #[cfg(any(target_os = "linux",target_os = "android"))]
    /// Is a file descriptor for a Linux dma_buf.
    /// It owns a reference to the underlying memory resource represented by its Vulkan memory object.
    DmaBuf,
    #[cfg(target_os = "android")]
    /// Specifies an AHardwareBuffer object defined by the Android NDK. See Android Hardware Buffers for more details of this handle type.
    AndroidHardwareBuffer,
    /// Specifies a host pointer returned by a host memory allocation command.
    /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the host memory is freed.
    HostAllocation,
    /// Specifies a host pointer to host mapped foreign memory.
    /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the foreign memory is unmapped or otherwise becomes no longer available.
    HostMappedForeignMemory,
}

