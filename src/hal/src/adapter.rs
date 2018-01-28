//! Physical devices and adapter.
//!
//! Physical devices are the main entry point for opening a [Device](../struct.Device).

use {format, memory, Backend, Gpu, Features, Limits};
use queue::{Capability, QueueGroup};

/// Scheduling hint for devices about the priority of a queue.  Values range from `0.0` (low) to
/// `1.0` (high).
pub type QueuePriority = f32;

///
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MemoryTypeId(pub usize);

impl From<usize> for MemoryTypeId {
    fn from(id: usize) -> Self {
        MemoryTypeId(id)
    }
}

///
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MemoryType {
    /// Properties of the associated memory.
    pub properties: memory::Properties,
    /// Index to the underlying memory heap in `Gpu::memory_heaps`
    pub heap_index: usize,
}

/// Types of memory supported by this adapter and available memory.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MemoryProperties {
    /// Each memory type is associated with one heap of `memory_heaps`.
    /// Multiple types can point to the same heap.
    pub memory_types: Vec<MemoryType>,
    /// Memory heaps with their size in bytes.
    pub memory_heaps: Vec<u64>,
}

/// Represents a physical or virtual device, which is capable of running the backend.
pub trait PhysicalDevice<B: Backend>: Sized {
    /// Create a new logical device.
    ///
    /// # Errors
    ///
    /// - Returns `TooManyObjects` if the implementation can't create a new logical device.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate gfx_backend_empty as empty;
    /// # extern crate gfx_hal;
    /// # fn main() {
    /// use gfx_hal::PhysicalDevice;
    ///
    /// # let physical_device: empty::PhysicalDevice = return;
    /// # let family: empty::QueueFamily = return;
    /// let gpu = physical_device.open(vec![(family, vec![1.0; 1])]);
    /// # }
    /// ```
    fn open(&self, Vec<(B::QueueFamily, Vec<QueuePriority>)>) -> Result<Gpu<B>, DeviceCreationError>;

    ///
    fn format_properties(&self, Option<format::Format>) -> format::Properties;

    ///
    fn memory_properties(&self) -> MemoryProperties;

    /// Returns the features of this `Device`. This usually depends on the graphics API being
    /// used.
    fn get_features(&self) -> Features;

    /// Returns the limits of this `Device`.
    fn get_limits(&self) -> Limits;
}

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
    /// Use [`get_features`](trait.PhysicalDevice.html#tymethod.get_features)
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

/// Information about a backend adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AdapterInfo {
    /// Adapter name
    pub name: String,
    /// Vendor PCI id of the adapter
    pub vendor: usize,
    /// PCI id of the adapter
    pub device: usize,
    /// The device is based on a software rasterizer
    pub software_rendering: bool,
}

/// The list of `Adapter` instances is obtained by calling `Instance::enumerate_adapters()`.
///
/// Given an `Adapter` a `Gpu` can be constructed by calling `PhysicalDevice::open()` on its
/// `physical_device` field. However, if only a single queue family is needed, then the
/// `Adapter::open_with` convenience method can be used instead.
pub struct Adapter<B: Backend> {
    /// General information about this adapter.
    pub info: AdapterInfo,
    /// Actual physical device.
    pub physical_device: B::PhysicalDevice,
    /// Supported queue families information for this adapter.
    pub queue_families: Vec<B::QueueFamily>,
}

impl<B: Backend> Adapter<B> {
    /// Open the physical device with active queue families
    /// specified by a selector function with a specified queue capability.
    ///
    /// Selector returns `Some(count)` for the `count` number of queues
    /// to be created for a given queue family.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate gfx_backend_empty as empty;
    /// # extern crate gfx_hal as hal;
    /// use hal::General;
    /// # fn main() {
    ///
    /// # let adapter: hal::Adapter<empty::Backend> = return;
    /// let gpu = adapter.open_with::<_, General>(|_| Some(1));
    /// # }
    /// ```
    ///
    /// # Return
    ///
    /// Returns the same errors as `open` and `InitializationFailed` if no suitable
    /// queue family could be found.
    pub fn open_with<F, C>(mut self, selector: F) -> Result<(B::Device, QueueGroup<B, C>), DeviceCreationError>
    where
        F: Fn(&B::QueueFamily) -> Option<usize>,
        C: Capability,
    {
        use queue::QueueFamily;

        let requested_family = self.queue_families
            .drain(..)
            .flat_map(|family| {
                if C::supported_by(family.queue_type()) {
                    selector(&family)
                        .map(|count| {
                            assert!(count != 0 && count <= family.max_queues());
                            (family, vec![1.0; count])
                        })
                } else {
                    None
                }
            })
            .next();

        let (id, family) = match requested_family {
            Some((family, priorities)) => (family.id(), vec![(family, priorities)]),
            _ => return Err(DeviceCreationError::InitializationFailed),
        };

        let Gpu { device, mut queues } = self.physical_device.open(family)?;
        Ok((device, queues.take(id).unwrap()))
    }
}
