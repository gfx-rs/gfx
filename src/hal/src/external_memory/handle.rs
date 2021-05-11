
#[cfg(any(windows,doc))]
/// Windows handle
#[derive(Debug)]
pub struct Handle(*mut std::ffi::c_void);
#[cfg(any(windows,doc))]
impl From<*mut std::ffi::c_void> for Handle {
    fn from(ptr: *mut std::ffi::c_void)->Self {Self(ptr)}
}
#[cfg(any(windows,doc))]
impl std::ops::Deref for Handle {
    type Target = *mut std::ffi::c_void;
    fn deref(&self) -> &Self::Target {&self.0}
}


#[cfg(any(windows,doc))]
#[cfg_attr(feature = "unstable", doc(cfg(windows)))]
#[derive(Debug)]
#[allow(non_camel_case_types)]
/// External memory that rely on windows handles
pub enum ExternalMemoryHandle {
    /// Tmp
    OPAQUE_WIN32(Handle,u64),
    /// Tmp
    OPAQUE_WIN32_KMT(Handle,u64),
    /// Tmp. Size is ignored.
    D3D11_TEXTURE(Handle),
    /// Tmp. Size is ignored
    D3D11_TEXTURE_KMT(Handle),
    /// Tmp
    D3D12_HEAP(Handle,u64),
    /// Tmp
    D3D12_RESOURCE(Handle),
}
