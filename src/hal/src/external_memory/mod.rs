//! Structures related to the import external memory functionality

use crate::device::{AllocationError,OutOfMemory};

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory import error
pub enum ExternalMemoryImportError {
    /// Allocation error.
    #[error(transparent)]
    AllocationError(#[from] AllocationError),

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}
impl From<OutOfMemory> for ExternalMemoryImportError {
    fn from(error: OutOfMemory)->Self {Self::AllocationError(error.into())}
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory export error
pub enum ExternalMemoryExportError {
    /// Too many objects.
    #[error("Too many objects")]
    TooManyObjects,

    /// Out of host memory.
    #[error("Out of host memory")]
    OutOfHostMemory,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}

#[derive(Clone, Debug, PartialEq)]
/// External memory handle
pub enum ExternalMemoryHandle {
    /// Tmp
    OpaqueFd{
        /// File descriptor size
        size: u64
    },
    /// Tmp
    OpaqueWin32{
        /// File descriptor size
        size: u64
    },
    /// Tmp
    OpaqueWin32Kmt{
        /// File descriptor size
        size: u64
    },
    /// Tmp. Size is ignored.
    D3D11Texture,
    /// Tmp. Size is ignored
    D3D11TextureKmt,
    /// Tmp
    D3D12Heap{
        /// File descriptor size
        size: u64
    },
    /// Tmp
    D3D12Resource,
    /// Tmp
    DmaBuf{
        /// File descriptor size
        size: u64
    },
    /// Tmp
    AndroidHardwareBuffer{
        /// File descriptor size
        size: u64
    },
    /// Tmp
    HostAllocation{
        /// File descriptor size
        size: u64
    },
    /// Tmp
    HostMappedForeignMemory{
        /// File descriptor size
        size: u64
    },
}

/// External memory types
#[derive(Clone, Debug, PartialEq)]
pub enum ExternalMemoryType {
    /// specifies a POSIX file descriptor handle that has only limited valid usage outside of Vulkan and other compatible APIs.
    /// It must be compatible with the POSIX system calls dup, dup2, close, and the non-standard system call dup3.
    /// Additionally, it must be transportable over a socket using an SCM_RIGHTS control message.
    /// It owns a reference to the underlying memory resource represented by its Vulkan memory object.
    OpaqueFd,
    /// Specifies an NT handle that has only limited valid usage outside of Vulkan and other compatible APIs.
    /// It must be compatible with the functions DuplicateHandle, CloseHandle, CompareObjectHandles, GetHandleInformation, and SetHandleInformation.
    /// It owns a reference to the underlying memory resource represented by its Vulkan memory object.
    OpaqueWin32,
    /// Specifies a global share handle that has only limited valid usage outside of Vulkan and other compatible APIs.
    /// It is not compatible with any native APIs.
    /// It does not own a reference to the underlying memory resource represented by its Vulkan memory object, and will therefore become invalid when all Vulkan memory objects associated with it are destroyed.
    OpaqueWin32Kmt,
    /// Specifies an NT handle returned by IDXGIResource1::CreateSharedHandle referring to a Direct3D 10 or 11 texture resource.
    /// It owns a reference to the memory used by the Direct3D resource.
    D3D11Texture,
    /// Specifies a global share handle returned by IDXGIResource::GetSharedHandle referring to a Direct3D 10 or 11 texture resource.
    /// It does not own a reference to the underlying Direct3D resource, and will therefore become invalid when all Vulkan memory objects and Direct3D resources associated with it are destroyed.
    D3D11TextureKmt,
    /// Specifies an NT handle returned by ID3D12Device::CreateSharedHandle referring to a Direct3D 12 heap resource.
    /// It owns a reference to the resources used by the Direct3D heap.
    D3D12Heap,
    /// Specifies an NT handle returned by ID3D12Device::CreateSharedHandle referring to a Direct3D 12 committed resource.
    /// It owns a reference to the memory used by the Direct3D resource.
    D3D12Resource,
    /// Is a file descriptor for a Linux dma_buf.
    /// It owns a reference to the underlying memory resource represented by its Vulkan memory object.
    DmaBuf,
    /// Specifies an AHardwareBuffer object defined by the Android NDK. See Android Hardware Buffers for more details of this handle type.
    AndroidHardwareBuffer,
    /// Specifies a host pointer returned by a host memory allocation command.
    /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the host memory is freed.
    HostAllocation,
    /// Specifies a host pointer to host mapped foreign memory.
    /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the foreign memory is unmapped or otherwise becomes no longer available.
    HostMappedForeignMemory,
}
