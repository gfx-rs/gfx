//! Logical device adapters.
//!
//! Adapters are the main entry point for opening a [Device](../struct.Device).

use {Backend, Gpu};

/// Represents a physical or virtual device, which is capable of running the backend.
///
/// The `Adapter` is typically obtained from objects implementing `gfx::WindowExt` or
/// `gfx::Headless`. How these types are created is backend-specific.
pub trait Adapter<B: Backend>: Sized {
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
    /// # let adapter: empty::Adapter = return;
    /// let gpu = adapter.open();
    /// # }
    /// ```
    fn open(&self) -> Gpu<B>;

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

    /// Return the supported queue families for this adapter.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate gfx_backend_empty as empty;
    /// # extern crate gfx_hal;
    /// # fn main() {
    /// use gfx_hal::Adapter;
    /// use gfx_hal::queue::RawQueueFamily;
    ///
    /// # let adapter: empty::Adapter = return;
    /// for (i, qf) in adapter.queue_families().into_iter().enumerate() {
    ///     println!("Queue family ({:?}) type: {:?}", i, qf.queue_type());
    /// }
    /// # }
    /// ```
    fn queue_families(&self) -> &[&B::QueueFamily];
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
