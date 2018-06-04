//! The HAL prelude
//!
//! The `prelude` module specifies common traits, mods, structs, enums, and fn's to be included
//! into the current namespace using a wildcard.
//! 
//! To use this prelude:
//! ```rust,ignore
//! use hal::prelude::*;
//! ```
//! 
//! See the [Rust Docs](https://doc.rust-lang.org/std/prelude/#other-preludes) for more information.

#[doc(no_inline)] pub use ::{Backend, Instance};
#[doc(no_inline)] pub use ::adapter::PhysicalDevice;
#[doc(no_inline)] pub use ::command::{Level, Shot, Submittable};
#[doc(no_inline)] pub use ::device::Device;
#[doc(no_inline)] pub use ::format::AsFormat;
#[doc(no_inline)] pub use ::memory::Pod;
#[doc(no_inline)] pub use ::pool::RawCommandPool;
#[doc(no_inline)] pub use ::queue::capability::{Capability, Supports, Upper};
#[doc(no_inline)] pub use ::queue::family::QueueFamily;
#[doc(no_inline)] pub use ::range::RangeArg;
#[doc(no_inline)] pub use ::window::{Surface, Swapchain};
