//! Structures related to the import external memory functionality

mod errors;
pub use errors::*;

#[cfg(any(unix, doc))]
#[cfg_attr(feature = "unstable", doc(cfg(unix)))]
mod fd;
#[cfg(any(unix, doc))]
#[cfg_attr(feature = "unstable", doc(cfg(unix)))]
pub use fd::*;

#[cfg(any(windows, doc))]
#[cfg_attr(feature = "unstable", doc(cfg(windows)))]
mod handle;
#[cfg(any(windows, doc))]
#[cfg_attr(feature = "unstable", doc(cfg(windows)))]
pub use handle::*;

mod ptr;
pub use ptr::*;

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
        self.exportable
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
            #[cfg(any(unix, doc))]
            ExternalMemoryType::Fd(external_memory_fd_type) => external_memory_fd_type.into(),
            #[cfg(any(windows, doc))]
            ExternalMemoryType::Handle(external_memory_handle_type) => {
                external_memory_handle_type.into()
            }
            ExternalMemoryType::Ptr(external_memory_ptr_type) => external_memory_ptr_type.into(),
        }
    }
}

/// External memory types
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum ExternalMemoryType {
    #[cfg(any(unix, doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
    /// External memory fd type
    Fd(ExternalMemoryFdType),
    #[cfg(any(windows, doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// External memory handle type
    Handle(ExternalMemoryHandleType),
    /// External memory ptr type
    Ptr(ExternalMemoryPtrType),
}

/// External memory handle
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum ExternalMemory {
    #[cfg(any(unix, doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
    /// External memory fd
    Fd(ExternalMemoryFd),
    #[cfg(any(windows, doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// External memory handle
    Handle(ExternalMemoryHandle),
    /// External memory ptr
    Ptr(ExternalMemoryPtr),
}
impl ExternalMemory {
    /// Get the size of this external memory
    pub fn get_size(&self) -> u64 {
        match self {
            #[cfg(any(unix, doc))]
            Self::Fd(external_memory_fd) => external_memory_fd.get_size(),
            #[cfg(any(windows, doc))]
            Self::Handle(external_memory_handle) => external_memory_handle.get_size(),
            Self::Ptr(external_memory_ptr) => external_memory_ptr.get_size(),
        }
    }
    /// Get the type of this external memory
    pub fn get_type(&self) -> ExternalMemoryType {
        match self {
            #[cfg(any(unix, doc))]
            Self::Fd(external_memory_fd) => ExternalMemoryType::Fd(external_memory_fd.get_type()),
            #[cfg(any(windows, doc))]
            Self::Handle(external_memory_handle) => {
                ExternalMemoryType::Handle(external_memory_handle.get_type())
            }
            Self::Ptr(external_memory_ptr) => {
                ExternalMemoryType::Ptr(external_memory_ptr.get_type())
            }
        }
    }
}
