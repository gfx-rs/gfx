//! Structures and enums related to external memory errors

use crate::buffer::CreationError;
use crate::device::{AllocationError, OutOfMemory};

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
/// External memory import error
pub enum ExternalMemoryImportError {
    /// Allocation error.
    #[error(transparent)]
    AllocationError(#[from] AllocationError),

    /// Creation error.
    #[error(transparent)]
    CreationError(#[from] CreationError),

    /// Invalid external handle.
    #[error("Invalid external handle")]
    InvalidExternalHandle,

    /// Unsupported parameters.
    #[error("Unsupported parameters")]
    UnsupportedParameters,

    /// Unsupported feature.
    #[error("Unsupported feature")]
    UnsupportedFeature,
}

impl From<OutOfMemory> for ExternalMemoryImportError {
    fn from(error: OutOfMemory) -> Self {
        Self::AllocationError(error.into())
    }
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
