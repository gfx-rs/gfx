//! Return values from function calls.

/// Device creation errors during `open`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceCreationError {
    /// Memory allocation on the host side failed.
    /// This could be caused by a lack of memory.
    OutOfHostMemory,
    /// Memory allocation on the device side failed.
    /// This could be caused by a lack of memory.
    OutOfDeviceMemory,
    /// Device initialization failed due to implementation specific errors.
    InitializationFailed,
    /// At least one of the user requested extensions if not supported by the
    /// physical device.
    MissingExtension,
    /// At least one of the user requested features if not supported by the
    /// physical device.
    ///
    /// Use [`features`](trait.PhysicalDevice.html#tymethod.features)
    /// for checking the supported features.
    MissingFeature,
    /// Too many logical devices have been created from this physical device.
    ///
    /// The implementation may only support one logical device for each physical
    /// device or lacks resources to allocate a new device.
    TooManyObjects,
    /// The logical or physical device are lost during the device creation
    /// process.
    ///
    /// This may be caused by hardware failure, physical device removal,
    /// power outage, etc.
    DeviceLost,
}

/// Errors during execution of operations on the host side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostExecutionError {
    /// Memory allocation on the host side failed.
    /// This could be caused by a lack of memory.
    OutOfHostMemory,
    /// Memory allocation on the device side failed.
    /// This could be caused by a lack of memory.
    OutOfDeviceMemory,
    /// The logical or physical device are lost during the device creation
    /// process.
    ///
    /// This may be caused by hardware failure, physical device removal,
    /// power outage, etc.
    DeviceLost,
}
