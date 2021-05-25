//! Structures and enums related to external memory errors

use crate::device::OutOfMemory;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory import error
pub enum ExternalBufferQueryError {
    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory import error
pub enum ExternalImageQueryError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),

    /// Requested image format not supported in combination with other parameters.
    #[error("Format not supported")]
    FormatNotSupported,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External image drm format query error
pub enum ExternalImageDrmFormatQueryError {
    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}



#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External buffer create error
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

    /// Requested binding to memory that doesn't support the required operations.
    #[error("Wrong memory")]
    WrongMemory,

    /// Requested binding to an invalid memory.
    #[error("Requested range is outside the resource")]
    OutOfBounds,

    /// Invalid external handle.
    #[error("The used external handle or the combination of them is invalid")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}


#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External image create error
pub enum ExternalImageCreateAllocateError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),

    /// Requested buffer usage is not supported.
    ///
    /// Older GL version don't support constant buffers or multiple usage flags.
    #[error("Unsupported usage: {0:?}")]
    UnsupportedUsage(crate::image::Usage),

    /// Cannot create any more objects.
    #[error("Too many objects")]
    TooManyObjects,

    /// Requested binding to memory that doesn't support the required operations.
    #[error("Wrong memory")]
    WrongMemory,

    /// Requested binding to an invalid memory.
    #[error("Requested range is outside the resource")]
    OutOfBounds,

    /// Invalid external handle.
    #[error("The used external handle or the combination of them is invalid")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory import error
pub enum ExternalBufferImportError {
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

    /// Requested binding to memory that doesn't support the required operations.
    #[error("Wrong memory")]
    WrongMemory,

    /// Requested binding to an invalid memory.
    #[error("Requested range is outside the resource")]
    OutOfBounds,

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}


#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory import error
pub enum ExternalImageImportError {
    /// Out of either host or device memory.
    #[error(transparent)]
    OutOfMemory(#[from] OutOfMemory),

    /// Requested buffer usage is not supported.
    ///
    /// Older GL version don't support constant buffers or multiple usage flags.
    #[error("Unsupported usage: {0:?}")]
    UnsupportedUsage(crate::image::Usage),

    /// Cannot create any more objects.
    #[error("Too many objects")]
    TooManyObjects,

    /// Requested binding to memory that doesn't support the required operations.
    #[error("Wrong memory")]
    WrongMemory,

    /// Requested binding to an invalid memory.
    #[error("Requested range is outside the resource")]
    OutOfBounds,

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
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

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}
