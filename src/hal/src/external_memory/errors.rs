//! Structures and enums related to external memory errors

use crate::buffer::CreationError;
use crate::device::{AllocationError,BindError, OutOfMemory};

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory import error
pub enum ExternalMemoryQueryError {
    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External buffer create error
pub enum ExternalBufferCreateError {
    /// Creation error.
    #[error(transparent)]
    CreationError(#[from] CreationError),

    /// Invalid external handle.
    #[error("The used external handle or the combination of them is invalid")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}
impl From<OutOfMemory> for ExternalBufferCreateError {
    fn from(error: OutOfMemory) -> Self {
        Self::CreationError(error.into())
    }
}

/*
impl From<CreationError> for ExternalBufferCreateError {
    fn from(error: CreationError)->Self {Self::CreationError(error.into())}
}
*/

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External buffer create error
pub enum ExternalBufferCreateAllocateError {
    /// Creation error.
    #[error(transparent)]
    CreationError(#[from] CreationError),

    /// Allocation error.
    #[error(transparent)]
    AllocationError(#[from] AllocationError),

    /// Bind error.
    #[error(transparent)]
    BindError(#[from] BindError),

    /// Invalid external handle.
    #[error("The used external handle or the combination of them is invalid")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}
/*
impl From<OutOfMemory> for ExternalBufferCreateAllocateError {
    fn from(error: OutOfMemory) -> Self {
        Self::CreationError(error.into())
    }
}
*/

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
pub enum ExternalMemoryAllocateError {
    /// Allocation error.
    #[error(transparent)]
    AllocationError(#[from] AllocationError),

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}

impl From<OutOfMemory> for ExternalMemoryAllocateError {
    fn from(error: OutOfMemory) -> Self {
        Self::AllocationError(error.into())
    }
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

    /// Unsupported parameters.
    #[error("Unsupported parameters")]
    UnsupportedParameters,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
/// External memory export error
pub enum ExternalMemoryError {
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
