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
    FragmentedPool,
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
        match result {
            ERROR_OUT_OF_HOST_MEMORY => Error::OutOfHostMemory,
            ERROR_OUT_OF_DEVICE_MEMORY => Error::OutOfDeviceMemory,
            ERROR_INITIALIZATION_FAILED => Error::InitializationFailed,
            ERROR_DEVICE_LOST => Error::DeviceLost,
            ERROR_MEMORY_MAP_FAILED => Error::MemoryMapFailed,
            ERROR_LAYER_NOT_PRESENT => Error::LayerNotPresent,
            ERROR_EXTENSION_NOT_PRESENT => Error::ExtensionNotPresent,
            ERROR_FEATURE_NOT_PRESENT => Error::FeatureNotPresent,
            ERROR_INCOMPATIBLE_DRIVER => Error::IncompatibleDriver,
            ERROR_TOO_MANY_OBJECTS => Error::TooManyObjects,
            ERROR_FORMAT_NOT_SUPPORTED => Error::FormatNotSupported,
            ERROR_FRAGMENTED_POOL => Error::FragmentedPool,
            ERROR_SURFACE_LOST_KHR => Error::SurfaceLostKhr,
            ERROR_NATIVE_WINDOW_IN_USE_KHR => Error::NativeWindowInUseKhr,
            ERROR_OUT_OF_DATE_KHR => Error::OutOfDateKhr,
            ERROR_INCOMPATIBLE_DISPLAY_KHR => Error::IncompatibleDisplayKhr,
            ERROR_VALIDATION_FAILED_EXT => Error::ValidationFailedExt,
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
