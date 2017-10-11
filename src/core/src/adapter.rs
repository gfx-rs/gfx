//! Logical device adapters.
//!
//! Adapters are the main entry point for opening a [Device](../struct.Device).

use {Backend, Gpu, QueueType};

/// Represents a physical or virtual device, which is capable of running the backend.
///
/// The `Adapter` is typically obtained from objects implementing `gfx::WindowExt` or
/// `gfx::Headless`. How these types are created is backend-specific.
pub trait Adapter<B: Backend>: Sized {
    /// Create a new logical gpu with the specified queues.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate empty;
    /// # extern crate gfx_core;
    /// # fn main() {
    /// use gfx_core::{Adapter, QueueFamily};
    ///
    /// # let adapter: empty::Adapter = return;
    /// let queue_desc = adapter.get_queue_families()
    ///                         .iter()
    ///                         .map(|&(ref family, ty)|
    ///                             (family, ty, family.num_queues()))
    ///                         .collect::<Vec<_>>();
    /// let gpu = adapter.open(&queue_desc);
    /// # }
    /// ```
    fn open(&self, queue_descs: &[(&B::QueueFamily, QueueType, u32)]) -> Gpu<B>;

    /// Create a new gpu with the specified queues.
    ///
    /// Takes an closure and creates the number of queues for each queue type
    /// as returned by the closure. Queues returning a number of 0 will be filtered out.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate empty;
    /// # extern crate gfx_core;
    /// # fn main() {
    /// use gfx_core::{Adapter, QueueType, Surface};
    ///
    /// # let adapter: empty::Adapter = return;
    /// # let surface: empty::Surface = return;
    /// // Open a gpu with a graphics queue, which can be used for presentation.
    /// // GeneralQueues will be downcasted to GraphicsQueues.
    /// let gpu = adapter.open_with(|family, ty| {
    ///     ((ty.supports_graphics() && surface.supports_queue(&family)) as u32, QueueType::Graphics)
    /// });
    /// # }
    /// ```
    fn open_with<F>(&self, mut f: F) -> Gpu<B>
    where
        F: FnMut(&B::QueueFamily, QueueType) -> (u32, QueueType),
    {
        let queue_desc = self.get_queue_families()
            .iter()
            .filter_map(|&(ref family, ty)| {
                let (num_queues, ty) = f(family, ty);
                if num_queues > 0 {
                    Some((family, ty, num_queues))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        self.open(&queue_desc)
    }

    /// Get the `AdapterInfo` for this adapter.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate empty;
    /// # extern crate gfx_core;
    /// # fn main() {
    /// use gfx_core::Adapter;
    ///
    /// # let adapter: empty::Adapter = return;
    /// println!("Adapter info: {:?}", adapter.get_info());
    /// # }
    /// ```
    fn get_info(&self) -> &AdapterInfo;

    /// Return the supported queue families for this adapter.
    ///
    /// * `QueueType` will be the one with the most capabilities.
    /// * There can be multiple families with the same queue type.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # extern crate empty;
    /// # extern crate gfx_core;
    /// # fn main() {
    /// use gfx_core::Adapter;
    ///
    /// # let adapter: empty::Adapter = return;
    /// for (i, &(_, ty)) in adapter.get_queue_families().into_iter().enumerate() {
    ///     println!("Queue family ({:?}) type: {:?}", i, ty);
    /// }
    /// # }
    /// ```
    fn get_queue_families(&self) -> &[(B::QueueFamily, QueueType)];
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
