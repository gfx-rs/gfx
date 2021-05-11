/// External memory handle
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum ExternalMemoryHandle {
    #[cfg(any(unix,doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(unix)))]
    /// Tmp
    OPAQUE_FD(Fd,u64),
    #[cfg(any(windows,doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    OPAQUE_WIN32(Handle,u64),
    #[cfg(any(windows,doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    OPAQUE_WIN32_KMT(Handle,u64),
    #[cfg(any(windows,doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp. Size is ignored.
    D3D11_TEXTURE(Handle),
    #[cfg(any(windows,doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp. Size is ignored
    D3D11_TEXTURE_KMT(Handle),
    #[cfg(any(windows,doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    D3D12_HEAP(Handle,u64),
    #[cfg(any(windows,doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(windows)))]
    /// Tmp
    D3D12_RESOURCE(Handle),
    #[cfg(any(target_os = "linux",target_os = "android",doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(any(target_os = "linux",target_os = "android"))))]
    /// Tmp
    DMA_BUF(Fd,u64),
    #[cfg(any(target_os = "android",doc))]
    #[cfg_attr(feature = "unstable", doc(cfg(target_os = "android")))]
    /// Tmp
    ANDROID_HARDWARE_BUFFER(Fd,u64),
    /// Tmp
    HOST_ALLOCATION(Ptr,u64),
    /// Tmp
    HOST_MAPPED_FOREIGN_MEMORY(Ptr,u64),
}
impl ExternalMemoryHandle {
    /// Get the external memory type from this handle
    pub fn get_type(&self)->ExternalMemoryType {
        match self{
            #[cfg(unix)]
            Self::OPAQUE_FD(_,_)=>ExternalMemoryType::OPAQUE_FD,
            #[cfg(windows)]
            Self::OPAQUE_WIN32(_,_)=>ExternalMemoryType::OPAQUE_WIN32,
            #[cfg(windows)]
            Self::OPAQUE_WIN32_KMT(_,_)=>ExternalMemoryType::OPAQUE_WIN32_KMT,
            #[cfg(windows)]
            Self::D3D11_TEXTURE(_)=>ExternalMemoryType::D3D11_TEXTURE,
            #[cfg(windows)]
            Self::D3D11_TEXTURE_KMT(_)=>ExternalMemoryType::D3D11_TEXTURE_KMT,
            #[cfg(windows)]
            Self::D3D12_HEAP(_,_)=>ExternalMemoryType::D3D12_HEAP,
            #[cfg(windows)]
            Self::D3D12_RESOURCE(_)=>ExternalMemoryType::D3D12_RESOURCE,
            #[cfg(any(target_os = "linux",target_os = "android"))]
            Self::DMA_BUF(_,_)=>ExternalMemoryType::DMA_BUF,
            #[cfg(target_os = "android")]
            Self::ANDROID_HARDWARE_BUFFER(_,_)=>ExternalMemoryType::ANDROID_HARDWARE_BUFFER,
            Self::HOST_ALLOCATION(_,_)=>ExternalMemoryType::HOST_ALLOCATION,
            Self::HOST_MAPPED_FOREIGN_MEMORY(_,_)=>ExternalMemoryType::HOST_MAPPED_FOREIGN_MEMORY,
        }
    }

    /// Get the size of this handle
    pub fn get_size(&self)->u64 {
        match self{
            #[cfg(unix)]
            Self::OPAQUE_FD(_,size)=>*size,
            #[cfg(windows)]
            Self::OPAQUE_WIN32(_,size)=>*size,
            #[cfg(windows)]
            Self::OPAQUE_WIN32_KMT(_,size)=>*size,
            #[cfg(windows)]
            Self::D3D11_TEXTURE(_)=>0,
            #[cfg(windows)]
            Self::D3D11_TEXTURE_KMT(_)=>0,
            #[cfg(windows)]
            Self::D3D12_HEAP(_,size)=>*size,
            #[cfg(windows)]
            Self::D3D12_RESOURCE(_)=>0,
            #[cfg(any(target_os = "linux",target_os = "android"))]
            Self::DMA_BUF(_,size)=>*size,
            #[cfg(target_os = "android")]
            Self::ANDROID_HARDWARE_BUFFER(_,size)=>*size,
            Self::HOST_ALLOCATION(_,size)=>*size,
            Self::HOST_MAPPED_FOREIGN_MEMORY(_,size)=>*size,
        }
    }

    pub fn get_external_handle(&self)->&ExternalHandle {
        match self{
            #[cfg(unix)]
            Self::OPAQUE_FD(fd,_)=>&ExternalHandle::Fd(*fd),
            #[cfg(windows)]
            Self::OPAQUE_WIN32(_,size)=>*size,
            #[cfg(windows)]
            Self::OPAQUE_WIN32_KMT(_,size)=>*size,
            #[cfg(windows)]
            Self::D3D11_TEXTURE(_)=>0,
            #[cfg(windows)]
            Self::D3D11_TEXTURE_KMT(_)=>0,
            #[cfg(windows)]
            Self::D3D12_HEAP(_,size)=>*size,
            #[cfg(windows)]
            Self::D3D12_RESOURCE(_)=>0,
            #[cfg(any(target_os = "linux",target_os = "android"))]
            Self::DMA_BUF(fd,_)=>&ExternalHandle::Fd(*fd),
            #[cfg(target_os = "android")]
            Self::ANDROID_HARDWARE_BUFFER(_,size)=>*size,
            //Self::HOST_ALLOCATION(_,size)=>*size,
            //Self::HOST_MAPPED_FOREIGN_MEMORY(_,size)=>*size,
            _=>unimplemented!()
        }
    }

    /// Extract the info in a tuple format
    pub fn extract(self)->(ExternalHandle,u64,ExternalMemoryType) {
        match self {
            #[cfg(unix)]
            ExternalMemoryHandle::OPAQUE_FD(fd,size)=>(fd.into(),size,ExternalMemoryType::OPAQUE_FD),
            #[cfg(windows)]
            ExternalMemoryHandle::OPAQUE_WIN32(handle,size)=>(handle.into(),size,ExternalMemoryType::OPAQUE_WIN32),
            #[cfg(windows)]
            ExternalMemoryHandle::OPAQUE_WIN32_KMT(handle,size)=>(handle.into(),size,ExternalMemoryType::OPAQUE_WIN32_KMT),
            #[cfg(windows)]
            ExternalMemoryHandle::D3D11_TEXTURE(handle)=>(handle.into(),0,ExternalMemoryType::D3D11_TEXTURE),
            #[cfg(windows)]
            ExternalMemoryHandle::D3D11_TEXTURE_KMT(handle)=>(handle.into(),0,ExternalMemoryType::D3D11_TEXTURE_KMT),
            #[cfg(windows)]
            ExternalMemoryHandle::D3D12_HEAP(handle,size)=>(handle.into(),size,ExternalMemoryType::D3D12_HEAP),
            #[cfg(windows)]
            ExternalMemoryHandle::D3D12_RESOURCE(handle)=>(handle.into(),0,ExternalMemoryType::D3D12_RESOURCE),
            #[cfg(any(target_os = "linux",target_os = "android"))]
            ExternalMemoryHandle::DMA_BUF(fd,size)=>(fd.into(),size,ExternalMemoryType::DMA_BUF),
            #[cfg(target_os = "android")]
            ExternalMemoryHandle::ANDROID_HARDWARE_BUFFER(fd,size)=>(fd.into(),size,ExternalMemoryType::ANDROID_HARDWARE_BUFFER),
            ExternalMemoryHandle::HOST_ALLOCATION(ptr,size)=>(ptr.into(),size,ExternalMemoryType::HOST_ALLOCATION),
            ExternalMemoryHandle::HOST_MAPPED_FOREIGN_MEMORY(ptr,size)=>(ptr.into(),size,ExternalMemoryType::HOST_MAPPED_FOREIGN_MEMORY),
        }
    }
}
