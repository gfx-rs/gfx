//! Physical devices and adapters.
//!
//! The `PhysicalDevice` trait specifies the API a backend must provide for dealing with
//! and querying a physical device, such as a particular GPU.  An `Adapter` is a struct
//! containing a `PhysicalDevice` and metadata for a particular GPU, generally created
//! from an `Instance` of that backend.  From an `Adapter` you can get a `Device` via
//! `adapter.open_with(...)`, which lets you specify properties of the `Device` you want.

use {format, memory, Backend, Gpu, Features, Limits};
use error::DeviceCreationError;
use queue::{Capability, QueueGroup};

/// Scheduling hint for devices about the priority of a queue.  Values range from `0.0` (low) to
/// `1.0` (high).
pub type QueuePriority = f32;

/// A strongly-typed index to a particular `MemoryType`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MemoryTypeId(pub usize);

impl From<usize> for MemoryTypeId {
    fn from(id: usize) -> Self {
        MemoryTypeId(id)
    }
}

/// A description for a single chunk of memory in a heap.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MemoryType {
    /// Properties of the associated memory, such as  synchronization
    /// properties or whether it's on the CPU or GPU.
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

/// Represents a physical device (such as a GPU) capable of supporting the given backend.
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

    /// Fetch details for a particular image format.
    fn format_properties(&self, Option<format::Format>) -> format::Properties;

    /// Fetch details for the memory regions provided by the device.
    fn memory_properties(&self) -> MemoryProperties;

    /// Returns the features of this `Device`. This usually depends on the graphics API being
    /// used.
    fn get_features(&self) -> Features;

    /// Returns the resource limits of this `Device`.
    fn get_limits(&self) -> Limits;
}

/// Metadata about a backend adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AdapterInfo {
    /// Adapter name
    pub name: String,
    /// Vendor PCI id of the adapter
    pub vendor: usize,
    /// PCI id of the adapter
    pub device: usize,
    /// Whether or not the device is based on a software rasterizer
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
    /// Queue families supported by this adapter.
    pub queue_families: Vec<B::QueueFamily>,
}

impl<B: Backend> Adapter<B> {
    /// Open the physical device with `count` queues from some active queue family. The family is
    /// the first that both provides the capability `C`, supports at least `count' queues, and for
    /// which `selector` returns true.
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
    /// let device = adapter.open_with::<_, General>(1, |_| true);
    /// # }
    /// ```
    ///
    /// # Return
    ///
    /// Returns the same errors as `open` and `InitializationFailed` if no suitable
    /// queue family could be found.
    pub fn open_with<F, C>(mut self, count: usize, selector: F) -> Result<(B::Device, QueueGroup<B, C>), DeviceCreationError>
    where
        F: Fn(&B::QueueFamily) -> bool,
        C: Capability,
    {
        use queue::QueueFamily;

        let requested_family = self.queue_families
            .drain(..)
            .filter(|family| {
                C::supported_by(family.queue_type()) &&
                    selector(&family) &&
                    count <= family.max_queues()
            })
            .next();

        let (id, family) = match requested_family {
            Some(family) => (family.id(), vec![(family, vec![1.0; count])]),
            _ => return Err(DeviceCreationError::InitializationFailed),
        };

        let Gpu { device, mut queues } = self.physical_device.open(family)?;
        Ok((device, queues.take(id).unwrap()))
    }
}
