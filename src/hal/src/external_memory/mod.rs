//! Structures related to the import external memory functionality

mod errors;
pub use errors::*;

pub use external_memory::*;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use drm_fourcc::*;

/// Buffer or image
#[derive(Debug)]
pub enum Resource<B: crate::Backend> {
    /// Buffer
    Buffer(B::Buffer),
    /// Image
    Image(B::Image),
}

/// External memory properties
#[derive(Debug, PartialEq)]
pub struct ExternalMemoryProperties {
    exportable: bool,
    importable: bool,
    exportable_from_imported: bool,
}
impl ExternalMemoryProperties {
    /// Constructor
    pub fn new(exportable: bool, importable: bool, exportable_from_imported: bool) -> Self {
        Self {
            exportable,
            importable,
            exportable_from_imported,
        }
    }
    /// Is the queried configuration exportable
    pub fn is_exportable(&self) -> bool {
        self.exportable
    }
    /// Is the queried configuration importable
    pub fn is_importable(&self) -> bool {
        self.importable
    }
    /// Does the queried configuration requires dedicated allocation
    pub fn is_exportable_from_imported(&self) -> bool {
        self.exportable_from_imported
    }
}
impl Default for ExternalMemoryProperties {
    fn default() -> Self {
        Self {
            exportable: false,
            importable: false,
            exportable_from_imported: false,
        }
    }
}

bitflags!(
    /// External memory type flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct ExternalMemoryTypeFlags: u32 {
        #[cfg(any(unix,doc))]
        /// This is supported on Unix only.
        /// Specifies a POSIX file descriptor handle that has only limited valid usage outside of Vulkan and other compatible APIs.
        /// It must be compatible with the POSIX system calls dup, dup2, close, and the non-standard system call dup3.
        /// Additionally, it must be transportable over a socket using an SCM_RIGHTS control message.
        /// It owns a reference to the underlying memory resource represented by its memory object.
        const OPAQUE_FD = (1 << 0);
        #[cfg(any(windows,doc))]
        /// This is supported on Windows only.
        /// Specifies an NT handle that has only limited valid usage outside of Vulkan and other compatible APIs.
        /// It must be compatible with the functions DuplicateHandle, CloseHandle, CompareObjectHandles, GetHandleInformation, and SetHandleInformation.
        /// It owns a reference to the underlying memory resource represented by its memory object.
        const OPAQUE_WIN32 = (1 << 1);
        #[cfg(any(windows,doc))]
        /// This is supported on Windows only.
        /// Specifies a global share handle that has only limited valid usage outside of Vulkan and other compatible APIs.
        /// It is not compatible with any native APIs.
        /// It does not own a reference to the underlying memory resource represented by its memory object, and will therefore become invalid when all the memory objects with it are destroyed.
        const OPAQUE_WIN32_KMT = (1 << 2);
        #[cfg(any(windows,doc))]
        /// This is supported on Windows only.
        /// Specifies an NT handle returned by IDXGIResource1::CreateSharedHandle referring to a Direct3D 10 or 11 texture resource.
        /// It owns a reference to the memory used by the Direct3D resource.
        const D3D11_TEXTURE = (1 << 3);
        #[cfg(any(windows,doc))]
        /// This is supported on Windows only.
        /// Specifies a global share handle returned by IDXGIResource::GetSharedHandle referring to a Direct3D 10 or 11 texture resource.
        /// It does not own a reference to the underlying Direct3D resource, and will therefore become invalid when all the memory objects and Direct3D resources associated with it are destroyed.
        const D3D11_TEXTURE_KMT = (1 << 4);
        #[cfg(any(windows,doc))]
        /// This is supported on Windows only.
        /// Specifies an NT handle returned by ID3D12Device::CreateSharedHandle referring to a Direct3D 12 heap resource.
        /// It owns a reference to the resources used by the Direct3D heap.
        const D3D12_HEAP = (1 << 5);
        #[cfg(any(windows,doc))]
        /// This is supported on Windows only.
        /// Specifies an NT handle returned by ID3D12Device::CreateSharedHandle referring to a Direct3D 12 committed resource.
        /// It owns a reference to the memory used by the Direct3D resource.
        const D3D12_RESOURCE = (1 << 6);
        #[cfg(any(target_os = "linux",target_os = "android",doc))]
        /// This is supported on Linux or Android only.
        /// Is a file descriptor for a Linux dma_buf.
        /// It owns a reference to the underlying memory resource represented by its memory object.
        const DMA_BUF = (1 << 7);
        #[cfg(any(target_os = "android",doc))]
        /// This is supported on Android only.
        /// Specifies an AHardwareBuffer object defined by the Android NDK. See Android Hardware Buffers for more details of this handle type.
        const ANDROID_HARDWARE_BUFFER = (1 << 8);
        /// Specifies a host pointer returned by a host memory allocation command.
        /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the host memory is freed.
        const HOST_ALLOCATION = (1 << 9);
        /// Specifies a host pointer to host mapped foreign memory.
        /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the foreign memory is unmapped or otherwise becomes no longer available.
        const HOST_MAPPED_FOREIGN_MEMORY = (1 << 10);
    }
);

impl From<ExternalMemoryType> for ExternalMemoryTypeFlags {
    fn from(external_memory_type: ExternalMemoryType) -> Self {
        match external_memory_type {
            #[cfg(unix)]
            ExternalMemoryType::OpaqueFd => Self::OPAQUE_FD,
            #[cfg(windows)]
            ExternalMemoryType::OpaqueWin32 => Self::OPAQUE_WIN32,
            #[cfg(windows)]
            ExternalMemoryType::OpaqueWin32Kmt => Self::OPAQUE_WIN32_KMT,
            #[cfg(windows)]
            ExternalMemoryType::D3D11Texture => Self::D3D11_TEXTURE,
            #[cfg(windows)]
            ExternalMemoryType::D3D11TextureKmt => Self::D3D11_TEXTURE_KMT,
            #[cfg(windows)]
            ExternalMemoryType::D3D12Heap => Self::D3D12_HEAP,
            #[cfg(windows)]
            ExternalMemoryType::D3D12Resource => Self::D3D12_RESOURCE,
            #[cfg(any(target_os = "linux", target_os = "android", doc))]
            ExternalMemoryType::DmaBuf => Self::DMA_BUF,
            #[cfg(target_os = "android")]
            ExternalMemoryType::AndroidHardwareBuffer => Self::ANDROID_HARDWARE_BUFFER,
            ExternalMemoryType::HostAllocation => Self::HOST_ALLOCATION,
            ExternalMemoryType::HostMappedForeignMemory => Self::HOST_MAPPED_FOREIGN_MEMORY,
        }
    }
}

/// External memory types
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExternalMemoryType {
    #[cfg(any(unix, doc))]
    /// This is supported on Unix only.
    /// Same as [ExternalMemoryTypeFlags::OPAQUE_FD][ExternalMemoryTypeFlags::OPAQUE_FD]
    OpaqueFd,
    #[cfg(any(windows, doc))]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::OPAQUE_WIN32][ExternalMemoryTypeFlags::OPAQUE_WIN32]
    OpaqueWin32,
    #[cfg(any(windows, doc))]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::OPAQUE_WIN32_KMT][ExternalMemoryTypeFlags::OPAQUE_WIN32_KMT]
    OpaqueWin32Kmt,
    #[cfg(any(windows, doc))]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::D3D11_TEXTURE][ExternalMemoryTypeFlags::D3D11_TEXTURE]
    D3D11Texture,
    #[cfg(any(windows, doc))]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::D3D11_TEXTURE_KMT][ExternalMemoryTypeFlags::D3D11_TEXTURE_KMT]
    D3D11TextureKmt,
    #[cfg(any(windows, doc))]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::D3D12_HEAP][ExternalMemoryTypeFlags::D3D12_HEAP]
    D3D12Heap,
    #[cfg(any(windows, doc))]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::D3D12_RESOURCE][ExternalMemoryTypeFlags::D3D12_RESOURCE]
    D3D12Resource,
    #[cfg(any(target_os = "linux", target_os = "android", doc))]
    /// This is supported on Linux or Android only.
    /// Same as [ExternalMemoryTypeFlags::DMA_BUF][ExternalMemoryTypeFlags::DMA_BUF]
    DmaBuf,
    #[cfg(any(target_os = "android", doc))]
    /// This is supported on Android only.
    /// Same as [ExternalMemoryTypeFlags::ANDROID_HARDWARE_BUFFER][ExternalMemoryTypeFlags::ANDROID_HARDWARE_BUFFER]
    AndroidHardwareBuffer,
    /// Same as [ExternalMemoryTypeFlags::HOST_ALLOCATION][ExternalMemoryTypeFlags::HOST_ALLOCATION]
    HostAllocation,
    /// Same as [ExternalMemoryTypeFlags::HOST_MAPPED_FOREIGN_MEMORY][ExternalMemoryTypeFlags::HOST_MAPPED_FOREIGN_MEMORY]
    HostMappedForeignMemory,
}

impl Into<PlatformMemoryType> for ExternalMemoryType {
    fn into(self) -> PlatformMemoryType {
        match self {
            #[cfg(unix)]
            ExternalMemoryType::OpaqueFd => PlatformMemoryType::Fd,
            #[cfg(windows)]
            ExternalMemoryType::OpaqueWin32 => PlatformMemoryType::Handle,
            #[cfg(windows)]
            ExternalMemoryType::OpaqueWin32Kmt => PlatformMemoryType::Handle,
            #[cfg(windows)]
            ExternalMemoryType::D3D11Texture => PlatformMemoryType::Handle,
            #[cfg(windows)]
            ExternalMemoryType::D3D11TextureKmt => PlatformMemoryType::Handle,
            #[cfg(windows)]
            ExternalMemoryType::D3D12Heap => PlatformMemoryType::Handle,
            #[cfg(windows)]
            ExternalMemoryType::D3D12Resource => PlatformMemoryType::Handle,
            #[cfg(any(target_os = "linux", target_os = "android", doc))]
            ExternalMemoryType::DmaBuf => PlatformMemoryType::Fd,
            #[cfg(any(target_os = "android", doc))]
            ExternalMemoryType::AndroidHardwareBuffer => PlatformMemoryType::Fd,
            ExternalMemoryType::HostAllocation => PlatformMemoryType::Ptr,
            ExternalMemoryType::HostMappedForeignMemory => PlatformMemoryType::Ptr,
        }
    }
}

/// External memory handle
#[derive(Debug)]
pub enum ExternalMemory {
    #[cfg(unix)]
    /// Tmp
    OpaqueFd(Fd),
    #[cfg(windows)]
    /// Tmp
    OpaqueWin32(Handle),
    #[cfg(windows)]
    /// Tmp
    OpaqueWin32Kmt(Handle),
    #[cfg(windows)]
    /// Tmp. Size is ignored.
    D3D11Texture(Handle),
    #[cfg(windows)]
    /// Tmp. Size is ignored
    D3D11TextureKmt(Handle),
    #[cfg(windows)]
    /// Tmp
    D3D12Heap(Handle),
    #[cfg(windows)]
    /// Tmp. Size is ignored
    D3D12Resource(Handle),
    #[cfg(any(target_os = "linux", target_os = "android", doc))]
    /// Tmp
    DmaBuf(Fd),
    #[cfg(any(target_os = "android", doc))]
    /// Tmp
    AndroidHardwareBuffer(Fd),
    /// Tmp
    HostAllocation(Ptr),
    /// Tmp
    HostMappedForeignMemory(Ptr),
}
impl ExternalMemory {
    /// Get the type of this external memory
    pub fn external_memory_type(&self) -> ExternalMemoryType {
        match self {
            #[cfg(unix)]
            Self::OpaqueFd(_) => ExternalMemoryType::OpaqueFd,
            #[cfg(windows)]
            Self::OpaqueWin32(_) => ExternalMemoryType::OpaqueWin32,
            #[cfg(windows)]
            Self::OpaqueWin32Kmt(_) => ExternalMemoryType::OpaqueWin32Kmt,
            #[cfg(windows)]
            Self::D3D11Texture(_) => ExternalMemoryType::D3D11Texture,
            #[cfg(windows)]
            Self::D3D11TextureKmt(_) => ExternalMemoryType::D3D11TextureKmt,
            #[cfg(windows)]
            Self::D3D12Heap(_) => ExternalMemoryType::D3D12Heap,
            #[cfg(windows)]
            Self::D3D12Resource(_) => ExternalMemoryType::D3D12Resource,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Self::DmaBuf(_) => ExternalMemoryType::DmaBuf,
            #[cfg(target_os = "android")]
            Self::AndroidHardwareBuffer(_) => ExternalMemoryType::AndroidHardwareBuffer,
            Self::HostAllocation(_) => ExternalMemoryType::HostAllocation,
            Self::HostMappedForeignMemory(_) => ExternalMemoryType::HostMappedForeignMemory,
        }
    }

    /// Get the type of this external memory
    pub fn platform_memory_type(&self) -> PlatformMemoryType {
        match self {
            #[cfg(unix)]
            Self::OpaqueFd(_) => PlatformMemoryType::Fd,
            #[cfg(windows)]
            Self::OpaqueWin32(_) => PlatformMemoryType::Handle,
            #[cfg(windows)]
            Self::OpaqueWin32Kmt(_) => PlatformMemoryType::Handle,
            #[cfg(windows)]
            Self::D3D11Texture(_) => PlatformMemoryType::Handle,
            #[cfg(windows)]
            Self::D3D11TextureKmt(_) => PlatformMemoryType::Handle,
            #[cfg(windows)]
            Self::D3D12Heap(_) => PlatformMemoryType::Handle,
            #[cfg(windows)]
            Self::D3D12Resource(_) => PlatformMemoryType::Handle,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Self::DmaBuf(_) => PlatformMemoryType::Fd,
            #[cfg(target_os = "android")]
            Self::AndroidHardwareBuffer(_) => PlatformMemoryType::Fd,
            Self::HostAllocation(_) => PlatformMemoryType::Ptr,
            Self::HostMappedForeignMemory(_) => PlatformMemoryType::Ptr,
        }
    }

    #[cfg(unix)]
    /// Get the unix file descriptor of this external memory
    pub fn fd(&self) -> Option<&Fd> {
        match self {
            Self::OpaqueFd(fd) => Some(fd),
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Self::DmaBuf(fd) => Some(fd),
            #[cfg(target_os = "android")]
            Self::AndroidHardwareBuffer(fd) => Some(fd),
            _ => None,
        }
    }
    #[cfg(windows)]
    /// Get the windows handle of this external memory
    pub fn handle(&self) -> Option<&Handle> {
        match self {
            Self::OpaqueWin32(handle) => Some(handle),
            Self::OpaqueWin32Kmt(handle) => Some(handle),
            Self::D3D11Texture(handle) => Some(handle),
            Self::D3D11TextureKmt(handle) => Some(handle),
            Self::D3D12Heap(handle) => Some(handle),
            Self::D3D12Resource(handle) => Some(handle),
            _ => None,
        }
    }

    /// Get the host pointer of this external memory
    pub fn ptr(&self) -> Option<&Ptr> {
        match self {
            Self::HostAllocation(ptr) => Some(ptr),
            Self::HostMappedForeignMemory(ptr) => Some(ptr),
            // Without this on non unix or windows platform, this will trigger error for unreachable pattern
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }
}

impl From<ExternalMemory> for (ExternalMemoryType, PlatformMemory) {
    fn from(external_memory: ExternalMemory) -> (ExternalMemoryType, PlatformMemory) {
        match external_memory {
            #[cfg(unix)]
            ExternalMemory::OpaqueFd(fd) => (ExternalMemoryType::OpaqueFd, fd.into()),
            #[cfg(windows)]
            ExternalMemory::OpaqueWin32(handle) => (ExternalMemoryType::OpaqueWin32, handle.into()),
            #[cfg(windows)]
            ExternalMemory::OpaqueWin32Kmt(handle) => {
                (ExternalMemoryType::OpaqueWin32Kmt, handle.into())
            }
            #[cfg(windows)]
            ExternalMemory::D3D11Texture(handle) => {
                (ExternalMemoryType::D3D11Texture, handle.into())
            }
            #[cfg(windows)]
            ExternalMemory::D3D11TextureKmt(handle) => {
                (ExternalMemoryType::D3D11TextureKmt, handle.into())
            }
            #[cfg(windows)]
            ExternalMemory::D3D12Heap(handle) => (ExternalMemoryType::D3D12Heap, handle.into()),
            #[cfg(windows)]
            ExternalMemory::D3D12Resource(handle) => {
                (ExternalMemoryType::D3D12Resource, handle.into())
            }
            #[cfg(any(target_os = "linux", target_os = "android"))]
            ExternalMemory::DmaBuf(fd) => (ExternalMemoryType::DmaBuf, fd.into()),
            #[cfg(target_os = "android")]
            ExternalMemory::AndroidHardwareBuffer(fd) => {
                (ExternalMemoryType::AndroidHardwareBuffer, fd.into())
            }
            ExternalMemory::HostAllocation(ptr) => (ExternalMemoryType::HostAllocation, ptr.into()),
            ExternalMemory::HostMappedForeignMemory(ptr) => {
                (ExternalMemoryType::HostMappedForeignMemory, ptr.into())
            }
        }
    }
}

impl std::convert::TryFrom<(ExternalMemoryType, PlatformMemory)> for ExternalMemory {
    type Error = &'static str;
    fn try_from(tuple: (ExternalMemoryType, PlatformMemory)) -> Result<Self, Self::Error> {
        match tuple {
            #[cfg(unix)]
            (ExternalMemoryType::OpaqueFd, PlatformMemory::Fd(fd)) => Ok(Self::OpaqueFd(fd)),
            #[cfg(windows)]
            (ExternalMemoryType::OpaqueWin32, PlatformMemory::Handle(handle)) => {
                Ok(Self::OpaqueWin32(handle))
            }
            #[cfg(windows)]
            (ExternalMemoryType::OpaqueWin32Kmt, PlatformMemory::Handle(handle)) => {
                Ok(Self::OpaqueWin32Kmt(handle))
            }
            #[cfg(windows)]
            (ExternalMemoryType::D3D11Texture, PlatformMemory::Handle(handle)) => {
                Ok(Self::D3D11Texture(handle))
            }
            #[cfg(windows)]
            (ExternalMemoryType::D3D11TextureKmt, PlatformMemory::Handle(handle)) => {
                Ok(Self::D3D11TextureKmt(handle))
            }
            #[cfg(windows)]
            (ExternalMemoryType::D3D12Heap, PlatformMemory::Handle(handle)) => {
                Ok(Self::D3D12Heap(handle))
            }
            #[cfg(windows)]
            (ExternalMemoryType::D3D12Resource, PlatformMemory::Handle(handle)) => {
                Ok(Self::D3D12Resource(handle))
            }
            #[cfg(any(target_os = "linux", target_os = "android"))]
            (ExternalMemoryType::DmaBuf, PlatformMemory::Fd(fd)) => Ok(Self::DmaBuf(fd)),
            #[cfg(target_os = "android")]
            (ExternalMemoryType::AndroidHardwareBuffer, PlatformMemory::Fd(fd)) => {
                Ok(Self::AndroidHardwareBuffer(fd))
            }
            (ExternalMemoryType::HostAllocation, PlatformMemory::Ptr(ptr)) => {
                Ok(Self::HostAllocation(ptr))
            }
            (ExternalMemoryType::HostMappedForeignMemory, PlatformMemory::Ptr(ptr)) => {
                Ok(Self::HostMappedForeignMemory(ptr))
            }
            // Without this on non unix or windows platform, this will trigger error for unreachable pattern
            #[allow(unreachable_patterns)]
            _ => Err("Wrong handle type and platform memory combination"),
        }
    }
}

#[cfg(unix)]
impl std::convert::TryFrom<(ExternalMemoryType, Fd)> for ExternalMemory {
    type Error = &'static str;
    fn try_from(tuple: (ExternalMemoryType, Fd)) -> Result<Self, Self::Error> {
        ExternalMemory::try_from((tuple.0, PlatformMemory::from(tuple.1)))
    }
}

#[cfg(windows)]
impl std::convert::TryFrom<(ExternalMemoryType, Handle)> for ExternalMemory {
    type Error = &'static str;
    fn try_from(tuple: (ExternalMemoryType, Handle)) -> Result<Self, Self::Error> {
        ExternalMemory::try_from((tuple.0, PlatformMemory::from(tuple.1)))
    }
}

impl std::convert::TryFrom<(ExternalMemoryType, Ptr)> for ExternalMemory {
    type Error = &'static str;
    fn try_from(tuple: (ExternalMemoryType, Ptr)) -> Result<Self, Self::Error> {
        ExternalMemory::try_from((tuple.0, PlatformMemory::from(tuple.1)))
    }
}

bitflags::bitflags! {
    /// Possible usages for an image.
    pub struct ImageUsage: u32 {
        /// Format can be used for sampled images (SAMPLED_IMAGE and COMBINED_IMAGE_SAMPLER descriptor types).
        const SAMPLED_IMAGE = (1 << 0);
        /// Format can be used for storage images (STORAGE_IMAGE descriptor type).
        const STORAGE_IMAGE = (1 << 1);
        /// Format supports atomic operations in case it is used for storage images.
        const STORAGE_IMAGE_ATOMIC = (1 << 2);
        /// Format can be used for uniform texel buffers (TBOs).
        const UNIFORM_TEXEL_BUFFER = (1 << 3);
        /// Format can be used for storage texel buffers (IBOs).
        const STORAGE_TEXEL_BUFFER = (1 << 4);
        /// Format supports atomic operations in case it is used for storage texel buffers.
        const STORAGE_TEXEL_BUFFER_ATOMIC = (1 << 5);
        /// Format can be used for vertex buffers (VBOs).
        const VERTEX_BUFFER = (1 << 6);
        /// Format can be used for color attachment images.
        const COLOR_ATTACHMENT = (1 << 7);
        /// Format supports blending in case it is used for color attachment images.
        const COLOR_ATTACHMENT_BLEND = (1 << 8);
        /// Format can be used for depth/stencil attachment images.
        const DEPTH_STENCIL_ATTACHMENT = (1 << 9);
        /// Format can be used as the source image of blits with vkCmdBlitImage.
        const BLIT_SRC = (1 << 10);
        /// Format can be used as the destination image of blits with vkCmdBlitImage.
        const BLIT_DST = (1 << 11);
        /// Format can be filtered with VK_FILTER_LINEAR when being sampled
        const SAMPLED_IMAGE_FILTER_LINEAR = (1 << 12);
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug)]
/// Drm format properties
pub struct DrmFormatProperties {
    /// Drm format modifier.
    pub drm_modifier: DrmModifier,
    /// Number of memory planes each image crated with `drm_modifier` has.
    pub plane_count: u32,
    /// Valid image usage with the `drm_modifier`.
    pub valid_usages: ImageUsage,
}
