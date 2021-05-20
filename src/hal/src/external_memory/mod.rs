//! Structures related to the import external memory functionality

mod errors;
pub use errors::*;

pub use external_memory::*;

/// Buffer or image
#[derive(Debug)]
pub enum BufferOrImage<'a, B: crate::Backend> {
    /// Buffer
    Buffer(&'a B::Buffer),
    /// Image
    Image(&'a B::Image),
}

/// External buffer properties
#[derive(Debug, PartialEq)]
pub struct ExternalBufferProperties {
    usage: crate::buffer::Usage,
    sparse: crate::memory::SparseFlags,
    external_memory_properties: ExternalMemoryProperties,
}
impl ExternalBufferProperties {
    /// Constructor
    pub fn new(
        usage: crate::buffer::Usage,
        sparse: crate::memory::SparseFlags,
        external_memory_properties: ExternalMemoryProperties,
    ) -> Self {
        Self {
            usage,
            sparse,
            external_memory_properties,
        }
    }
    /// Is the queried configuration exportable
    pub fn get_queried_buffer_usage(&self) -> crate::buffer::Usage {
        self.usage
    }
    /// Is the queried configuration importable
    pub fn get_queried_buffer_sparse(&self) -> crate::memory::SparseFlags {
        self.sparse
    }
    /// Get external memory properties
    pub fn get_external_memory_properties(&self) -> &ExternalMemoryProperties {
        &self.external_memory_properties
    }
}
impl AsRef<ExternalMemoryProperties> for ExternalBufferProperties {
    fn as_ref(&self) -> &ExternalMemoryProperties {
        &self.external_memory_properties
    }
}
impl std::ops::Deref for ExternalBufferProperties {
    type Target = ExternalMemoryProperties;
    fn deref(&self) -> &Self::Target {
        &self.external_memory_properties
    }
}

/// External memory properties
#[derive(Debug, PartialEq)]
pub struct ExternalMemoryProperties {
    exportable: bool,
    importable: bool,
    dedicated_allocation: bool,
    memory_type: ExternalMemoryType,
    compatible_memory_types: ExternalMemoryTypeFlags,
    export_from_imported_memory_types: ExternalMemoryTypeFlags,
}
impl ExternalMemoryProperties {
    /// Constructor
    pub fn new(
        exportable: bool,
        importable: bool,
        dedicated_allocation: bool,
        memory_type: ExternalMemoryType,
        compatible_memory_types: ExternalMemoryTypeFlags,
        export_from_imported_memory_types: ExternalMemoryTypeFlags,
    ) -> Self {
        Self {
            exportable,
            importable,
            dedicated_allocation,
            memory_type,
            compatible_memory_types,
            export_from_imported_memory_types,
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
    pub fn requires_dedicated_allocation(&self) -> bool {
        self.dedicated_allocation
    }
    /// Get the queried memory type
    pub fn get_queried_memory_type(&self) -> ExternalMemoryType {
        self.memory_type
    }
    /// Get the external handle types compatible with the queried one
    pub fn get_compatile_memory_types(&self) -> ExternalMemoryTypeFlags {
        self.compatible_memory_types
    }
    /// Get the external handle types that can be exported from an imported memory using the queried external handle type
    pub fn get_export_from_imported_memory_types(&self) -> ExternalMemoryTypeFlags {
        self.export_from_imported_memory_types
    }
}

bitflags!(
    /// External memory type flags.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct ExternalMemoryTypeFlags: u32 {
        #[cfg(any(unix,doc))]
        #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
        /// Specifies a POSIX file descriptor handle that has only limited valid usage outside of Vulkan and other compatible APIs.
        /// It must be compatible with the POSIX system calls dup, dup2, close, and the non-standard system call dup3.
        /// Additionally, it must be transportable over a socket using an SCM_RIGHTS control message.
        /// It owns a reference to the underlying memory resource represented by its memory object.
        const OPAQUE_FD = 1;
        #[cfg(any(windows,doc))]
        #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
        /// Specifies an NT handle that has only limited valid usage outside of Vulkan and other compatible APIs.
        /// It must be compatible with the functions DuplicateHandle, CloseHandle, CompareObjectHandles, GetHandleInformation, and SetHandleInformation.
        /// It owns a reference to the underlying memory resource represented by its memory object.
        const OPAQUE_WIN32 = 2;
        #[cfg(any(windows,doc))]
        #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
        /// Specifies a global share handle that has only limited valid usage outside of Vulkan and other compatible APIs.
        /// It is not compatible with any native APIs.
        /// It does not own a reference to the underlying memory resource represented by its memory object, and will therefore become invalid when all the memory objects with it are destroyed.
        const OPAQUE_WIN32_KMT = 4;
        #[cfg(any(windows,doc))]
        #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
        /// Specifies an NT handle returned by IDXGIResource1::CreateSharedHandle referring to a Direct3D 10 or 11 texture resource.
        /// It owns a reference to the memory used by the Direct3D resource.
        const D3D11_TEXTURE = 8;
        #[cfg(any(windows,doc))]
        #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
        /// Specifies a global share handle returned by IDXGIResource::GetSharedHandle referring to a Direct3D 10 or 11 texture resource.
        /// It does not own a reference to the underlying Direct3D resource, and will therefore become invalid when all the memory objects and Direct3D resources associated with it are destroyed.
        const D3D11_TEXTURE_KMT = 16;
        #[cfg(any(windows,doc))]
        #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
        /// Specifies an NT handle returned by ID3D12Device::CreateSharedHandle referring to a Direct3D 12 heap resource.
        /// It owns a reference to the resources used by the Direct3D heap.
        const D3D12_HEAP = 32;
        #[cfg(any(windows,doc))]
        #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
        /// Specifies an NT handle returned by ID3D12Device::CreateSharedHandle referring to a Direct3D 12 committed resource.
        /// It owns a reference to the memory used by the Direct3D resource.
        const D3D12_RESOURCE = 64;
        #[cfg(any(target_os = "linux",target_os = "android",doc))]
        #[cfg_attr(feature = "unstable", doc(cfg(any(target_os = "linux",target_os = "android"))))]
        /// Is a file descriptor for a Linux dma_buf.
        /// It owns a reference to the underlying memory resource represented by its Vulkan memory object.
        const DMA_BUF = 128;
        #[cfg(any(target_os = "android",doc))]
        #[cfg_attr(feature = "unstable", doc(cfg(target_os = "android")))]
        /// Specifies an AHardwareBuffer object defined by the Android NDK. See Android Hardware Buffers for more details of this handle type.
        const ANDROID_HARDWARE_BUFFER = 256;
        /// Specifies a host pointer returned by a host memory allocation command.
        /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the host memory is freed.
        const HOST_ALLOCATION = 512;
        /// Specifies a host pointer to host mapped foreign memory.
        /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the foreign memory is unmapped or otherwise becomes no longer available.
        const HOST_MAPPED_FOREIGN_MEMORY = 1024;
    }
);

impl From<ExternalMemoryType> for ExternalMemoryTypeFlags {
    fn from(external_memory_type: ExternalMemoryType) -> Self {
        match external_memory_type {
            #[cfg(unix)]
            #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
            ExternalMemoryType::OpaqueFd => Self::OPAQUE_FD,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::OpaqueWin32 => Self::OPAQUE_WIN32,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::OpaqueWin32Kmt => Self::OPAQUE_WIN32_KMT,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::D3D11Texture => Self::D3D11_TEXTURE,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::D3D11TextureKmt => Self::D3D11_TEXTURE_KMT,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::D3D12Heap => Self::D3D12_HEAP,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::D3D12Resource => Self::D3D12_RESOURCE,
            #[cfg(any(target_os = "linux", target_os = "android", doc))]
            #[cfg_attr(
                feature = "unstable",
                doc(cfg(any(target_os = "linux", target_os = "android")))
            )]
            ExternalMemoryType::DmaBuf => Self::DMA_BUF,
            #[cfg(any(target_os = "android", doc))]
            #[cfg_attr(feature = "unstable", doc(cfg(target_os = "android")))]
            ExternalMemoryType::AndroidHardwareBuffer => Self::ANDROID_HARDWARE_BUFFER,
            ExternalMemoryType::HostAllocation => Self::HOST_ALLOCATION,
            ExternalMemoryType::HostMappedForeignMemory => Self::HOST_MAPPED_FOREIGN_MEMORY,
        }
    }
}

/// External memory types
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExternalMemoryType {
    #[cfg(unix)]
    #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
    /// Tmp
    OpaqueFd,
    #[cfg(windows)]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    OpaqueWin32,
    #[cfg(windows)]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    OpaqueWin32Kmt,
    #[cfg(windows)]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp. Size is ignored.
    D3D11Texture,
    #[cfg(windows)]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp. Size is ignored
    D3D11TextureKmt,
    #[cfg(windows)]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    D3D12Heap,
    #[cfg(windows)]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    D3D12Resource,
    #[cfg(any(target_os = "linux", target_os = "android", doc))]
    #[cfg_attr(
        feature = "unstable",
        doc(cfg(any(target_os = "linux", target_os = "android")))
    )]
    /// Tmp
    DmaBuf,
    #[cfg(any(target_os = "android", doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(target_os = "android")))]
    /// Tmp
    AndroidHardwareBuffer,
    /// Tmp
    HostAllocation,
    /// Tmp
    HostMappedForeignMemory,
}

impl Into<PlatformMemoryType> for ExternalMemoryType {
    fn into(self) -> PlatformMemoryType {
        match self {
            #[cfg(unix)]
            #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
            ExternalMemoryType::OpaqueFd => PlatformMemoryType::Fd,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::OpaqueWin32 => PlatformMemoryType::Handle,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::OpaqueWin32Kmt => PlatformMemoryType::Handle,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::D3D11Texture => PlatformMemoryType::Handle,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::D3D11TextureKmt => PlatformMemoryType::Handle,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::D3D12Heap => PlatformMemoryType::Handle,
            #[cfg(windows)]
            #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
            ExternalMemoryType::D3D12Resource => PlatformMemoryType::Handle,
            #[cfg(any(target_os = "linux", target_os = "android", doc))]
            #[cfg_attr(
                feature = "unstable",
                doc(cfg(any(target_os = "linux", target_os = "android")))
            )]
            ExternalMemoryType::DmaBuf => PlatformMemoryType::Fd,
            #[cfg(any(target_os = "android", doc))]
            #[cfg_attr(feature = "unstable", doc(cfg(target_os = "android")))]
            ExternalMemoryType::AndroidHardwareBuffer => PlatformMemoryType::Fd,
            ExternalMemoryType::HostAllocation => PlatformMemoryType::Ptr,
            ExternalMemoryType::HostMappedForeignMemory => PlatformMemoryType::Ptr,
        }
    }
}

/// External memory handle
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum ExternalMemory {
    #[cfg(any(unix))]
    #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
    /// Tmp
    OpaqueFd(Fd),
    #[cfg(any(windows))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    OpaqueWin32(Handle),
    #[cfg(any(windows))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    OpaqueWin32Kmt(Handle),
    #[cfg(any(windows))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp. Size is ignored.
    D3D11Texture(Handle),
    #[cfg(any(windows))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp. Size is ignored
    D3D11TextureKmt(Handle),
    #[cfg(any(windows))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    D3D12Heap(Handle),
    #[cfg(any(windows))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp. Size is ignored
    D3D12Resource(Handle),
    #[cfg(any(target_os = "linux", target_os = "android", doc))]
    #[cfg_attr(
        feature = "unstable",
        doc(cfg(any(target_os = "linux", target_os = "android")))
    )]
    /// Tmp
    DmaBuf(Fd),
    #[cfg(any(target_os = "android", doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(target_os = "android")))]
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
            #[cfg(any(unix))]
            Self::OpaqueFd(_) => ExternalMemoryType::OpaqueFd,
            #[cfg(any(windows))]
            Self::OpaqueWin32(_) => ExternalMemoryType::OpaqueWin32,
            #[cfg(any(windows))]
            Self::OpaqueWin32Kmt(_) => ExternalMemoryType::OpaqueWin32Kmt,
            #[cfg(any(windows))]
            Self::D3D11Texture(_) => ExternalMemoryType::D3D11Texture,
            #[cfg(any(windows))]
            Self::D3D11TextureKmt(_) => ExternalMemoryType::D3D11TextureKmt,
            #[cfg(any(windows))]
            Self::D3D12Heap(_) => ExternalMemoryType::D3D12Heap,
            #[cfg(any(windows))]
            Self::D3D12Resource(_) => ExternalMemoryType::D3D12Resource,
            #[cfg(any(target_os = "linux", target_os = "android", doc))]
            Self::DmaBuf(_) => ExternalMemoryType::DmaBuf,
            #[cfg(any(target_os = "android", doc))]
            Self::AndroidHardwareBuffer(_) => ExternalMemoryType::AndroidHardwareBuffer,
            Self::HostAllocation(_) => ExternalMemoryType::HostAllocation,
            Self::HostMappedForeignMemory(_) => ExternalMemoryType::HostMappedForeignMemory,
        }
    }

    /// Get the type of this external memory
    pub fn platform_memory_type(&self) -> PlatformMemoryType {
        match self {
            #[cfg(any(unix))]
            Self::OpaqueFd(_) => PlatformMemoryType::Fd,
            #[cfg(any(windows))]
            Self::OpaqueWin32(_) => PlatformMemoryType::Handle,
            #[cfg(any(windows))]
            Self::OpaqueWin32Kmt(_) => PlatformMemoryType::Handle,
            #[cfg(any(windows))]
            Self::D3D11Texture(_) => PlatformMemoryType::Handle,
            #[cfg(any(windows))]
            Self::D3D11TextureKmt(_) => PlatformMemoryType::Handle,
            #[cfg(any(windows))]
            Self::D3D12Heap(_) => PlatformMemoryType::Handle,
            #[cfg(any(windows))]
            Self::D3D12Resource(_) => PlatformMemoryType::Handle,
            #[cfg(any(target_os = "linux", target_os = "android", doc))]
            Self::DmaBuf(_) => PlatformMemoryType::Fd,
            #[cfg(any(target_os = "android", doc))]
            Self::AndroidHardwareBuffer(_) => PlatformMemoryType::Fd,
            Self::HostAllocation(_) => PlatformMemoryType::Ptr,
            Self::HostMappedForeignMemory(_) => PlatformMemoryType::Ptr,
        }
    }

    #[cfg(any(unix))]
    /// Get the unix file descriptor of this external memory
    pub fn fd(&self) -> Option<&Fd> {
        match self {
            Self::OpaqueFd(fd) => Some(fd),
            #[cfg(any(target_os = "linux", target_os = "android", doc))]
            Self::DmaBuf(fd) => Some(fd),
            #[cfg(any(target_os = "android", doc))]
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
            #[cfg(any(unix))]
            ExternalMemory::OpaqueFd(fd) => (ExternalMemoryType::OpaqueFd, fd.into()),
            #[cfg(any(windows))]
            ExternalMemory::OpaqueWin32(handle) => (ExternalMemoryType::OpaqueWin32, handle.into()),
            #[cfg(any(windows))]
            ExternalMemory::OpaqueWin32Kmt(handle) => {
                (ExternalMemoryType::OpaqueWin32Kmt, handle.into())
            }
            #[cfg(any(windows))]
            ExternalMemory::D3D11Texture(handle) => {
                (ExternalMemoryType::D3D11Texture, handle.into())
            }
            #[cfg(any(windows))]
            ExternalMemory::D3D11TextureKmt(handle) => {
                (ExternalMemoryType::D3D11TextureKmt, handle.into())
            }
            #[cfg(any(windows))]
            ExternalMemory::D3D12Heap(handle) => (ExternalMemoryType::D3D12Heap, handle.into()),
            #[cfg(any(windows))]
            ExternalMemory::D3D12Resource(handle) => {
                (ExternalMemoryType::D3D12Resource, handle.into())
            }
            #[cfg(any(target_os = "linux", target_os = "android", doc))]
            ExternalMemory::DmaBuf(fd) => (ExternalMemoryType::DmaBuf, fd.into()),
            #[cfg(any(target_os = "android", doc))]
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
            #[cfg(any(unix))]
            (ExternalMemoryType::OpaqueFd, PlatformMemory::Fd(fd)) => Ok(Self::OpaqueFd(fd)),
            #[cfg(any(windows))]
            (ExternalMemoryType::OpaqueWin32, PlatformMemory::Handle(handle)) => {
                Ok(Self::OpaqueWin32(handle))
            }
            #[cfg(any(windows))]
            (ExternalMemoryType::OpaqueWin32Kmt, PlatformMemory::Handle(handle)) => {
                Ok(Self::OpaqueWin32Kmt(handle))
            }
            #[cfg(any(windows))]
            (ExternalMemoryType::D3D11Texture, PlatformMemory::Handle(handle)) => {
                Ok(Self::D3D11Texture(handle))
            }
            #[cfg(any(windows))]
            (ExternalMemoryType::D3D11TextureKmt, PlatformMemory::Handle(handle)) => {
                Ok(Self::D3D11TextureKmt(handle))
            }
            #[cfg(any(windows))]
            (ExternalMemoryType::D3D12Heap, PlatformMemory::Handle(handle)) => {
                Ok(Self::D3D12Heap(handle))
            }
            #[cfg(any(windows))]
            (ExternalMemoryType::D3D12Resource, PlatformMemory::Handle(handle)) => {
                Ok(Self::D3D12Resource(handle))
            }
            #[cfg(any(target_os = "linux", target_os = "android", doc))]
            (ExternalMemoryType::DmaBuf, PlatformMemory::Fd(fd)) => Ok(Self::DmaBuf(fd)),
            #[cfg(any(target_os = "android", doc))]
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
