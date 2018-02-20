//! Return values from function calls.

/// Device creation errors during `open`.
#[derive(Fail, Debug, Clone, PartialEq, Eq)]
pub enum DeviceCreationError {
    /// Memory allocation on the host side failed.
    /// This could be caused by a lack of memory.
    #[fail(display = "Host memory allocation failed.")]
    OutOfHostMemory,
    /// Memory allocation on the device side failed.
    /// This could be caused by a lack of memory.
    #[fail(display = "Device memory allocation failed.")]
    OutOfDeviceMemory,
    /// Device initialization failed due to implementation specific errors.
    #[fail(display = "Device initialization failed.")]
    InitializationFailed,
    /// At least one of the user requested extensions if not supported by the
    /// physical device.
    #[fail(display = "One or multiple extensions are not supported.")]
    MissingExtension,
    /// At least one of the user requested features if not supported by the
    /// physical device.
    ///
    /// Use [`features`](trait.PhysicalDevice.html#tymethod.features)
    /// for checking the supported features.
    #[fail(display = "One or multiple features are not supported.")]
    MissingFeature,
    /// Too many logical devices have been created from this physical device.
    ///
    /// The implementation may only support one logical device for each physical
    /// device or lacks resources to allocate a new device.
    #[fail(display = "Too many device objects have been created.")]
    TooManyObjects,
    /// The logical or physical device are lost during the device creation
    /// process.
    ///
    /// This may be caused by hardware failure, physical device removal,
    /// power outage, etc.
    #[fail(display = "Physical or logical device lost.")]
    DeviceLost,
}

/// Errors during execution of operations on the host side.
#[derive(Fail, Debug, Clone, PartialEq, Eq)]
pub enum HostExecutionError {
    /// Memory allocation on the host side failed.
    /// This could be caused by a lack of memory.
    #[fail(display = "Host memory allocation failed.")]
    OutOfHostMemory,
    /// Memory allocation on the device side failed.
    /// This could be caused by a lack of memory.
    #[fail(display = "Device memory allocation failed.")]
    OutOfDeviceMemory,
    /// The logical or physical device are lost during the device creation
    /// process.
    ///
    /// This may be caused by hardware failure, physical device removal,
    /// power outage, etc.
    #[fail(display = "Physical or logical device lost.")]
    DeviceLost,
}
