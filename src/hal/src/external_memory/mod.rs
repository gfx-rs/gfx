//! Structures related to the import external memory functionality.

mod errors;
pub use errors::*;

pub use external_memory::*;

bitflags!(
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    /// External memory properties.
    pub struct ExternalMemoryProperties: u32 {
        /// The memory can be exported using [Device::export_memory][Device::export_memory].
        const EXPORTABLE = (1 << 0);
        /// The memory can be imported using [Device::import_external_image][Device::import_external_image] and [Device::import_external_buffer][Device::import_external_buffer].
        const IMPORTABLE = (1 << 1);
        /// The memory created using [Device::import_external_image][Device::import_external_image] and [Device::import_external_buffer][Device::import_external_buffer] can be exported using [Device::export_memory][Device::export_memory].
        const EXPORTABLE_FROM_IMPORTED = (1 << 2);
    }
);

/// Representation of an external memory for image creation.
#[derive(Debug)]
pub enum ExternalImageMemory {
    #[cfg(unix)]
    /// This is supported on Unix only.
    /// Same as [ExternalMemoryTypeFlags::OPAQUE_FD][ExternalMemoryTypeFlags::OPAQUE_FD] while holding a [Fd][Fd].
    OpaqueFd(Fd),
    #[cfg(windows)]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::OPAQUE_WIN32][ExternalMemoryTypeFlags::OPAQUE_WIN32] while holding a [Handle][Handle].
    OpaqueWin32(Handle),
    #[cfg(windows)]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::OPAQUE_WIN32_KMT][ExternalMemoryTypeFlags::OPAQUE_WIN32_KMT] while holding a [Handle][Handle].
    OpaqueWin32Kmt(Handle),
    #[cfg(windows)]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::D3D11_TEXTURE][ExternalMemoryTypeFlags::D3D11_TEXTURE] while holding a [Handle][Handle].
    D3D11Texture(Handle),
    #[cfg(windows)]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::D3D11_TEXTURE_KMT][ExternalMemoryTypeFlags::D3D11_TEXTURE_KMT] while holding a [Handle][Handle].
    D3D11TextureKmt(Handle),
    #[cfg(windows)]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::D3D12_HEAP][ExternalMemoryTypeFlags::D3D12_HEAP] while holding a [Handle][Handle].
    D3D12Heap(Handle),
    #[cfg(windows)]
    /// This is supported on Windows only.
    /// Same as [ExternalMemoryTypeFlags::D3D12_RESOURCE][ExternalMemoryTypeFlags::D3D12_RESOURCE] while holding a [Handle][Handle].
    D3D12Resource(Handle),
    #[cfg(any(target_os = "linux", target_os = "android"))]
    /// This is supported on Linux or Android only.
    /// Same as [ExternalMemoryTypeFlags::DMA_BUF][ExternalMemoryTypeFlags::DMA_BUF] while holding a [Fd][Fd].
    DmaBuf(Fd, Option<crate::image::DrmFormatImageProperties>),
    #[cfg(any(target_os = "android"))]
    /// This is supported on Android only.
    /// Same as [ExternalMemoryTypeFlags::ANDROID_HARDWARE_BUFFER][ExternalMemoryTypeFlags::ANDROID_HARDWARE_BUFFER] while holding a [Fd][Fd].
    AndroidHardwareBuffer(Fd),
    /// Same as [ExternalMemoryTypeFlags::HOST_ALLOCATION][ExternalMemoryTypeFlags::HOST_ALLOCATION] while holding a [Ptr][Ptr].
    HostAllocation(Ptr),
    /// Same as [ExternalMemoryTypeFlags::HOST_MAPPED_FOREIGN_MEMORY][ExternalMemoryTypeFlags::HOST_MAPPED_FOREIGN_MEMORY] while holding a [Ptr][Ptr].
    HostMappedForeignMemory(Ptr),
}
impl ExternalImageMemory {
    /// Get the [ExternalMemoryType][ExternalMemoryType] from this enum.
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
            Self::DmaBuf(_, _) => ExternalMemoryType::DmaBuf,
            #[cfg(target_os = "android")]
            Self::AndroidHardwareBuffer(_) => ExternalMemoryType::AndroidHardwareBuffer,
            Self::HostAllocation(_) => ExternalMemoryType::HostAllocation,
            Self::HostMappedForeignMemory(_) => ExternalMemoryType::HostMappedForeignMemory,
        }
    }

    /// Get the [PlatformMemoryType][PlatformMemoryType] from this enum.
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
            Self::DmaBuf(_, _) => PlatformMemoryType::Fd,
            #[cfg(target_os = "android")]
            Self::AndroidHardwareBuffer(_) => PlatformMemoryType::Fd,
            Self::HostAllocation(_) => PlatformMemoryType::Ptr,
            Self::HostMappedForeignMemory(_) => PlatformMemoryType::Ptr,
        }
    }

    #[cfg(unix)]
    /// Get the associated unix file descriptor as ([Fd][Fd]).
    pub fn fd(&self) -> Option<&Fd> {
        match self {
            Self::OpaqueFd(fd) => Some(fd),
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Self::DmaBuf(fd, _drm_format_properties) => Some(fd),
            #[cfg(target_os = "android")]
            Self::AndroidHardwareBuffer(fd) => Some(fd),
            _ => None,
        }
    }

    #[cfg(windows)]
    /// Get the associated windows handle as ([Handle][Handle]).
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

    /// Get the associated host pointer as ([Ptr][Ptr]).
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

