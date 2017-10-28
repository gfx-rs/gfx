//! Physical devices and adapter.
//!
//! Physical devices are the main entry point for opening a [Device](../struct.Device).

use {Backend, Gpu};

/// Scheduling hint for devices about the priority of a queue.
/// Values ranging from `0.0` (low) to `1.0` (high).
pub type QueuePriority = f32;

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
