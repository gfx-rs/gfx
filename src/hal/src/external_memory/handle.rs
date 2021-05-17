use super::{ExternalMemory, ExternalMemoryType, ExternalMemoryTypeFlags};

/// Windows handle
#[derive(Debug)]
pub struct Handle(*mut std::ffi::c_void);
impl From<*mut std::ffi::c_void> for Handle {
    fn from(ptr: *mut std::ffi::c_void) -> Self {
        Self(ptr)
    }
}
impl std::os::windows::io::AsRawHandle for Handle {
    fn as_raw_handle(&self) -> std::os::windows::raw::HANDLE {
        self.0
    }
}
impl std::ops::Deref for Handle {
    type Target = *mut std::ffi::c_void;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
/// External memory that rely on windows handles
pub enum ExternalMemoryHandle {
    /// Tmp
    OPAQUE_WIN32(Handle, u64),
    /// Tmp
    OPAQUE_WIN32_KMT(Handle, u64),
    /// Tmp. Size is ignored.
    D3D11_TEXTURE(Handle),
    /// Tmp. Size is ignored
    D3D11_TEXTURE_KMT(Handle),
    /// Tmp
    D3D12_HEAP(Handle, u64),
    /// Tmp. Size is ignored
    D3D12_RESOURCE(Handle),
}
impl ExternalMemoryHandle {
    /// Get the windows handle
    pub fn get_handle(&self) -> &Handle {
        match self {
            Self::OPAQUE_WIN32(handle, _) => handle,
            Self::OPAQUE_WIN32_KMT(handle, _) => handle,
            Self::D3D11_TEXTURE(handle) => handle,
            Self::D3D11_TEXTURE_KMT(handle) => handle,
            Self::D3D12_HEAP(handle, _) => handle,
            Self::D3D12_RESOURCE(handle) => handle,
        }
    }
    /// Get the size
    pub fn get_size(&self) -> u64 {
        match self {
            Self::OPAQUE_WIN32(_, size) => *size,
            Self::OPAQUE_WIN32_KMT(_, size) => *size,
            Self::D3D11_TEXTURE(_) => 0,
            Self::D3D11_TEXTURE_KMT(_) => 0,
            Self::D3D12_HEAP(_, size) => *size,
            Self::D3D12_RESOURCE(_) => 0,
        }
    }
    /// Get the external memory handle type
    pub fn get_type(&self) -> ExternalMemoryHandleType {
        match self {
            Self::OPAQUE_WIN32(_, _) => ExternalMemoryHandleType::OPAQUE_WIN32,
            Self::OPAQUE_WIN32_KMT(_, _) => ExternalMemoryHandleType::OPAQUE_WIN32_KMT,
            Self::D3D11_TEXTURE(_) => ExternalMemoryHandleType::D3D11_TEXTURE,
            Self::D3D11_TEXTURE_KMT(_) => ExternalMemoryHandleType::D3D11_TEXTURE_KMT,
            Self::D3D12_HEAP(_, _) => ExternalMemoryHandleType::D3D12_HEAP,
            Self::D3D12_RESOURCE(_) => ExternalMemoryHandleType::D3D12_RESOURCE,
        }
    }
}
impl From<(ExternalMemoryHandleType, Handle, u64)> for ExternalMemoryHandle {
    fn from(tuple: (ExternalMemoryHandleType, Handle, u64)) -> Self {
        match tuple.0 {
            ExternalMemoryHandleType::OPAQUE_WIN32 => Self::OPAQUE_WIN32(tuple.1, tuple.2),
            ExternalMemoryHandleType::OPAQUE_WIN32_KMT => Self::OPAQUE_WIN32_KMT(tuple.1, tuple.2),
            ExternalMemoryHandleType::D3D11_TEXTURE => Self::D3D11_TEXTURE(tuple.1),
            ExternalMemoryHandleType::D3D11_TEXTURE_KMT => Self::D3D11_TEXTURE_KMT(tuple.1),
            ExternalMemoryHandleType::D3D12_HEAP => Self::D3D12_HEAP(tuple.1, tuple.2),
            ExternalMemoryHandleType::D3D12_RESOURCE => Self::D3D12_RESOURCE(tuple.1),
        }
    }
}

impl Into<(ExternalMemoryHandleType, Handle, u64)> for ExternalMemoryHandle {
    fn into(self) -> (ExternalMemoryHandleType, Handle, u64) {
        match self {
            Self::OPAQUE_WIN32(handle, size) =>(ExternalMemoryHandleType::OPAQUE_WIN32, handle, size),
            Self::OPAQUE_WIN32_KMT(handle, size) => (ExternalMemoryHandleType::OPAQUE_WIN32_KMT, handle, size),
            Self::D3D11_TEXTURE(handle) => (ExternalMemoryHandleType::D3D11_TEXTURE, handle, 0),
            Self::D3D11_TEXTURE_KMT(handle) => (ExternalMemoryHandleType::D3D11_TEXTURE_KMT, handle, 0),
            Self::D3D12_HEAP(handle, size) => (ExternalMemoryHandleType::D3D12_HEAP, handle, size),
            Self::D3D12_RESOURCE(handle) => (ExternalMemoryHandleType::D3D12_RESOURCE, handle, 0),
        }
    }
}
impl From<ExternalMemoryHandle> for ExternalMemory {
    fn from(external_memory_handle: ExternalMemoryHandle) -> Self {
        Self::Handle(external_memory_handle)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(non_camel_case_types)]
/// External memory that rely on windows handles
pub enum ExternalMemoryHandleType {
    /// Tmp
    OPAQUE_WIN32,
    /// Tmp
    OPAQUE_WIN32_KMT,
    /// Tmp. Size is ignored.
    D3D11_TEXTURE,
    /// Tmp. Size is ignored
    D3D11_TEXTURE_KMT,
    /// Tmp
    D3D12_HEAP,
    /// Tmp
    D3D12_RESOURCE,
}

impl From<ExternalMemoryHandleType> for ExternalMemoryTypeFlags {
    fn from(external_memory_handle_type: ExternalMemoryHandleType) -> Self {
        match external_memory_handle_type {
            ExternalMemoryHandleType::OPAQUE_WIN32 => Self::OPAQUE_WIN32,
            ExternalMemoryHandleType::OPAQUE_WIN32_KMT => Self::OPAQUE_WIN32_KMT,
            ExternalMemoryHandleType::D3D11_TEXTURE => Self::D3D11_TEXTURE,
            ExternalMemoryHandleType::D3D11_TEXTURE_KMT => Self::D3D11_TEXTURE_KMT,
            ExternalMemoryHandleType::D3D12_HEAP => Self::D3D12_HEAP,
            ExternalMemoryHandleType::D3D12_RESOURCE => Self::D3D12_RESOURCE,
        }
    }
}
impl From<ExternalMemoryHandleType> for ExternalMemoryType {
    fn from(external_memory_handle_type: ExternalMemoryHandleType) -> Self {
        Self::Handle(external_memory_handle_type)
    }
}
