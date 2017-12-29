//! Physical devices and adapter.
//!
//! Physical devices are the main entry point for opening a [Device](../struct.Device).

use {format, memory, Backend, Gpu, Features, Limits};

/// Scheduling hint for devices about the priority of a queue.
/// Values ranging from `0.0` (low) to `1.0` (high).
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
    /// Create a new logical GPU.
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
    fn open(self, Vec<(B::QueueFamily, Vec<QueuePriority>)>) -> Gpu<B>;

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
    /// specified by a selector function.
    ///
    /// Selector returns `Some(count)` for the `count` number of queues
    /// to be created for a given queue family.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate gfx_backend_empty as empty;
    /// # extern crate gfx_hal;
    /// # fn main() {
    ///
    /// # let adapter: gfx_hal::Adapter<empty::Backend> = return;
    /// let gpu = adapter.open_with(|_| Some(1));
    /// # }
    /// ```
    pub fn open_with<F>(mut self, selector: F) -> Gpu<B>
    where F: Fn(&B::QueueFamily) -> Option<usize>
    {
        use queue::QueueFamily;

        let requested_families = self.queue_families
            .drain(..)
            .flat_map(|family| {
                selector(&family)
                    .map(|count| {
                        assert!(count != 0 && count <= family.max_queues());
                        (family, vec![1.0; count])
                    })
            })
            .collect();

        self.physical_device.open(requested_families)
    }
}
