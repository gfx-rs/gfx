//! Structures and enums related to external memory errors.

use crate::device::OutOfMemory;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// Error while enumerating external image properties. Returned from [PhysicalDevice::external_image_properties][crate::adapter::PhysicalDevice::external_image_properties].
pub enum ExternalImagePropertiesError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),

    /// Requested image format not supported in combination with other parameters.
    #[error("Format not supported")]
    FormatNotSupported,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// Error while creating and allocating an external buffer. Returned from [Device::create_allocate_external_buffer][crate::device::Device::create_allocate_external_buffer].
pub enum ExternalBufferCreateAllocateError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),

    /// Requested buffer usage is not supported.
    ///
    /// Older GL version don't support constant buffers or multiple usage flags.
    #[error("Unsupported usage: {0:?}")]
    UnsupportedUsage(crate::buffer::Usage),

    /// Cannot create any more objects.
    #[error("Too many objects")]
    TooManyObjects,

    /// All the desired memory type ids are invalid for the implementation..
    #[error("No valid memory type id among the desired ones")]
    NoValidMemoryTypeId,

    /// Invalid external handle.
    #[error("The used external handle or the combination of them is invalid")]
    InvalidExternalHandle,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// Error while creating and allocating an external image. Returned from [Device::create_allocate_external_image][crate::device::Device::create_allocate_external_image].
pub enum ExternalImageCreateAllocateError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),

    /// Cannot create any more objects.
    #[error("Too many objects")]
    TooManyObjects,

    /// All the desired memory type ids are invalid for the implementation..
    #[error("No valid memory type id among the desired ones")]
    NoValidMemoryTypeId,

    /// Invalid external handle.
    #[error("The used external handle or the combination of them is invalid")]
    InvalidExternalHandle,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// Error while importing an external memory as buffer. Returned from [Device::import_external_buffer][crate::device::Device::import_external_buffer].
pub enum ExternalBufferImportError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),

    /// Cannot create any more objects.
    #[error("Too many objects")]
    TooManyObjects,

    /// All the desired memory type ids are invalid for the implementation..
    #[error("No valid memory type id among the desired ones")]
    NoValidMemoryTypeId,

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// Error while importing an external memory as image. Returned from [Device::import_external_image][crate::device::Device::import_external_image].
pub enum ExternalImageImportError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),

    /// Cannot create any more objects.
    #[error("Too many objects")]
    TooManyObjects,

    /// All the desired memory type ids are invalid for the implementation..
    #[error("No valid memory type id among the desired ones")]
    NoValidMemoryTypeId,

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// Error while exporting a memory. Returned from [Device::export_memory][crate::device::Device::export_memory].
pub enum ExternalMemoryExportError {
    /// Too many objects.
    #[error("Too many objects")]
    TooManyObjects,

    /// Out of host memory.
    #[error("Out of host memory")]
    OutOfHostMemory,

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,
}
