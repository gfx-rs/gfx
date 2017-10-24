//! Logical device adapters.
//!
//! Adapters are the main entry point for opening a [Device](../struct.Device).

use {Backend, Gpu};

/// Represents a physical or virtual device, which is capable of running the backend.
///
/// The list of `Adapter` instances is obtained by calling `Instance::enumerate_adapters()`.
pub trait Adapter<B: Backend>: Sized {
    /// Get the `AdapterInfo` for this adapter.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate gfx_backend_empty as empty;
    /// # extern crate gfx_hal;
    /// # fn main() {
    /// use gfx_hal::Adapter;
    ///
    /// # let adapter: empty::Adapter = return;
    /// println!("Adapter info: {:?}", adapter.info());
    /// # }
    /// ```
    fn info(&self) -> &AdapterInfo;

    /// Create a new logical GPU.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate gfx_backend_empty as empty;
    /// # extern crate gfx_hal;
    /// # fn main() {
    /// use gfx_hal::{Adapter};
    ///
    /// # let mut adapter: empty::Adapter = return;
    /// let family: empty::ProtoQueueFamily = return;
    /// let gpu = adapter.open(vec![(family, 1)]);
    /// # }
    /// ```
    fn open(self, Vec<(B::ProtoQueueFamily, usize)>) -> Gpu<B>;

    /// Return the supported queue families information for this adapter.
    ///
    /// *Note*: supposed to be called once. A subsequent call returns
    /// an empty vector.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate gfx_backend_empty as empty;
    /// # extern crate gfx_hal;
    /// # fn main() {
    /// use gfx_hal::{Adapter, ProtoQueueFamily};
    ///
    /// # let mut adapter: empty::Adapter = return;
    /// for (i, qf) in adapter.list_queue_families().into_iter().enumerate() {
    ///     println!("Queue families ({:?}) type: {:?}", i, qf.queue_type());
    /// }
    /// # }
    /// ```
    fn list_queue_families(&mut self) -> Vec<B::ProtoQueueFamily>;
}

/// Information about a backend adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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
