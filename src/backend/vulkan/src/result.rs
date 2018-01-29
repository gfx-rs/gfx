
use ash::vk;

use hal::error::{DeviceCreationError, HostExecutionError};

// Generic error codes from Vulkan
#[derive(Debug)]
pub(crate) enum Error {
    OutOfHostMemory,
    OutOfDeviceMemory,
    InitializationFailed,
    DeviceLost,
    MemoryMapFailed,
    LayerNotPresent,
    ExtensionNotPresent,
    FeatureNotPresent,
    IncompatibleDriver,
    TooManyObjects,
    FormatNotSupported,
    FragmentedPool ,
    SurfaceLostKhr,
    NativeWindowInUseKhr,
    OutOfDateKhr,
    IncompatibleDisplayKhr,
    ValidationFailedExt,
    // Not an actual vulkan error, but handle the case where an implementation
    // might return an unkown error.
    Unknown,
}

impl From<vk::Result> for Error {
    fn from(result: vk::Result) -> Self {
        use ash::vk::Result::*;
        match result {
            ErrorOutOfHostMemory => Error::OutOfHostMemory,
            ErrorOutOfDeviceMemory => Error::OutOfDeviceMemory,
            ErrorInitializationFailed => Error::InitializationFailed,
            ErrorDeviceLost => Error::DeviceLost,
            ErrorMemoryMapFailed => Error::MemoryMapFailed,
            ErrorLayerNotPresent => Error::LayerNotPresent,
            ErrorExtensionNotPresent => Error::ExtensionNotPresent,
            ErrorFeatureNotPresent => Error::FeatureNotPresent,
            ErrorIncompatibleDriver => Error::IncompatibleDriver,
            ErrorTooManyObjects => Error::TooManyObjects,
            ErrorFormatNotSupported => Error::FormatNotSupported,
            ErrorFragmentedPool => Error::FragmentedPool,
            ErrorSurfaceLostKhr => Error::SurfaceLostKhr,
            ErrorNativeWindowInUseKhr => Error::NativeWindowInUseKhr,
            ErrorOutOfDateKhr => Error::OutOfDateKhr,
            ErrorIncompatibleDisplayKhr => Error::IncompatibleDisplayKhr,
            ErrorValidationFailedExt => Error::ValidationFailedExt,
            _ => Error::Unknown,
        }
    }
}

// Impl `From<Error>` for various HAL error types.
//
// Syntax:
//    #HalError {
//       #VulkanError => #HalErrorVariant,
//    }
macro_rules! from_error {
    { $($name:ident { $($base_error:ident => $err:ident,)* },)* } => {
        $(
            impl From<Error> for $name {
                fn from(err: Error) -> Self {
                    match err {
                        $(
                            Error::$base_error => $name::$err,
                        )*
                        _ => unreachable!("Unexpected error code ({:?}). Non specification conformant driver.", err),
                    }
                }
            }
        )*
    }
}

from_error! {
    DeviceCreationError {
        OutOfHostMemory => OutOfHostMemory,
        OutOfDeviceMemory => OutOfDeviceMemory,
        InitializationFailed => InitializationFailed,
        ExtensionNotPresent => MissingExtension,
        FeatureNotPresent => MissingFeature,
        TooManyObjects => TooManyObjects,
        DeviceLost => DeviceLost,
    },
}

from_error! {
    HostExecutionError {
        OutOfHostMemory => OutOfHostMemory,
        OutOfDeviceMemory => OutOfDeviceMemory,
        DeviceLost => DeviceLost,
    },
}
