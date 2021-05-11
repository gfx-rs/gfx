use super::{ExternalMemoryTypeFlags,ExternalMemoryType};

/// Pointer to a host allocated memory
#[derive(Debug)]
pub struct Ptr(*mut std::ffi::c_void);
impl Ptr {
    /// Get the inner ptr
    pub fn as_raw_ptr(&self)->*mut std::ffi::c_void {self.0}
}
impl From<*mut std::ffi::c_void> for Ptr {
    fn from(ptr: *mut std::ffi::c_void)->Self {Self(ptr)}
}
impl std::ops::Deref for Ptr {
    type Target = *mut std::ffi::c_void;
    fn deref(&self) -> &Self::Target {&self.0}
}


#[derive(Debug)]
#[allow(non_camel_case_types)]
/// External memory that rely on host pointers
pub enum ExternalMemoryPtr {
    /// Tmp
    HOST_ALLOCATION(Ptr,u64),
    /// Tmp
    HOST_MAPPED_FOREIGN_MEMORY(Ptr,u64),
}
impl ExternalMemoryPtr {
    /// Get the fd
    pub fn get_ptr(&self)->&Ptr {
        match self {
            Self::HOST_ALLOCATION(ptr,_)=>ptr,
            Self::HOST_MAPPED_FOREIGN_MEMORY(ptr,_)=>ptr,
        }
    }
    /// Get the size
    pub fn get_size(&self)->u64 {
        match self {
            Self::HOST_ALLOCATION(_,size)=>*size,
            Self::HOST_MAPPED_FOREIGN_MEMORY(_,size)=>*size,
        }
    }
    /// Get the external memory ptr type
    pub fn get_external_memory_ptr_type(&self)->ExternalMemoryPtrType {
        match self {
            Self::HOST_ALLOCATION(_,_)=>ExternalMemoryPtrType::HOST_ALLOCATION,
            Self::HOST_MAPPED_FOREIGN_MEMORY(_,_)=>ExternalMemoryPtrType::HOST_MAPPED_FOREIGN_MEMORY,
        }
    }
}


/// Subgroup of ExternalMemoryType that export as ptr
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum ExternalMemoryPtrType {
    /// Specifies a host pointer returned by a host memory allocation command.
    /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the host memory is freed.
    HOST_ALLOCATION,
    /// Specifies a host pointer to host mapped foreign memory.
    /// It does not own a reference to the underlying memory resource, and will therefore become invalid if the foreign memory is unmapped or otherwise becomes no longer available.
    HOST_MAPPED_FOREIGN_MEMORY,
}
impl From<ExternalMemoryPtrType> for ExternalMemoryTypeFlags {
    fn from(external_memory_fd_type: ExternalMemoryPtrType)->Self {
        match external_memory_fd_type {
            ExternalMemoryPtrType::HOST_ALLOCATION=>Self::HOST_ALLOCATION,
            ExternalMemoryPtrType::HOST_MAPPED_FOREIGN_MEMORY=>Self::HOST_MAPPED_FOREIGN_MEMORY,
        }
    }
}
impl From<ExternalMemoryPtrType> for ExternalMemoryType {
    fn from(external_memory_ptr_type: ExternalMemoryPtrType)->Self {
        Self::Ptr(external_memory_ptr_type)
    }
}


/*
impl std::convert::TryFrom<ExternalMemoryTypeFlags> for ExternalMemoryPtrType {
    type Error = &'static str;
    fn try_from(external_memory_types: ExternalMemoryTypeFlags)->Result<Self, Self::Error> {
        match external_memory_types {
            ExternalMemoryTypeFlags::HOST_ALLOCATION=>Ok(ExternalMemoryPtrType::HOST_ALLOCATION),
            ExternalMemoryTypeFlags::HOST_MAPPED_FOREIGN_MEMORY=>Ok(ExternalMemoryPtrType::HOST_MAPPED_FOREIGN_MEMORY),
            _=>Err("Cannot convert")
        }
    }
}

impl std::convert::TryFrom<ExternalMemoryType> for ExternalMemoryPtrType {
    type Error = &'static str;
    fn try_from(external_memory_type: ExternalMemoryType)->Result<Self, Self::Error> {
        match external_memory_type {
            ExternalMemoryType::HOST_ALLOCATION=>Ok(Self::HOST_ALLOCATION),
            ExternalMemoryType::HOST_MAPPED_FOREIGN_MEMORY=>Ok(Self::HOST_MAPPED_FOREIGN_MEMORY),
            _=>Err("Cannot convert")
        }
    }
}*/
