//! OpenGL implementation of a device, striving to support OpenGL 2.0 with at
//! least VAOs, but using newer extensions when available.

#![allow(missing_docs, missing_copy_implementations)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
extern crate gfx_gl as gl;
extern crate gfx_hal as hal;
extern crate smallvec;
extern crate spirv_cross;
#[cfg(feature = "glutin")]
pub extern crate glutin;

use std::cell::Cell;
use std::fmt;
use std::sync::Arc;
use std::ops::Deref;
use std::thread::{self, ThreadId};

use hal::{error, image};
use hal::queue::{Queues, QueueFamilyId};

pub use self::device::Device;
pub use self::info::{Info, PlatformName, Version};

mod command;
mod conv;
mod device;
mod info;
mod native;
mod pool;
mod queue;
mod state;
mod window;

#[cfg(feature = "glutin")]
pub use window::glutin::{config_context, Headless, Surface, Swapchain};

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl hal::Backend for Backend {
    type PhysicalDevice = PhysicalDevice;
    type Device = Device;

    type Surface = Surface;
    type Swapchain = Swapchain;

    type QueueFamily = QueueFamily;
    type CommandQueue = queue::CommandQueue;
    type CommandBuffer = command::RawCommandBuffer;

    type Memory = native::Memory;
    type CommandPool = pool::RawCommandPool;

    type ShaderModule = native::ShaderModule;
    type RenderPass = native::RenderPass;
    type Framebuffer = native::FrameBuffer;

    type UnboundBuffer = device::UnboundBuffer;
    type Buffer = native::Buffer;
    type BufferView = native::BufferView;
    type UnboundImage = device::UnboundImage;
    type Image = native::Image;
    type ImageView = native::ImageView;
    type Sampler = native::FatSampler;

    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineLayout = native::PipelineLayout;
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type DescriptorPool = native::DescriptorPool;
    type DescriptorSet = native::DescriptorSet;

    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
    type QueryPool = ();
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
    NoError,
    InvalidEnum,
    InvalidValue,
    InvalidOperation,
    InvalidFramebufferOperation,
    OutOfMemory,
    UnknownError,
}

impl Error {
    pub fn from_error_code(error_code: gl::types::GLenum) -> Error {
        match error_code {
            gl::NO_ERROR                      => Error::NoError,
            gl::INVALID_ENUM                  => Error::InvalidEnum,
            gl::INVALID_VALUE                 => Error::InvalidValue,
            gl::INVALID_OPERATION             => Error::InvalidOperation,
            gl::INVALID_FRAMEBUFFER_OPERATION => Error::InvalidFramebufferOperation,
            gl::OUT_OF_MEMORY                 => Error::OutOfMemory,
            _                                 => Error::UnknownError,
        }
    }
}

/// Internal struct of shared data between the physical and logical device.
struct Share {
    context: gl::Gl,
    info: Info,
    features: hal::Features,
    legacy_features: info::LegacyFeatures,
    limits: hal::Limits,
    private_caps: info::PrivateCaps,
    // Indicates if there is an active logical device.
    open: Cell<bool>,
}

impl Share {
    /// Fails during a debug build if the implementation's error flag was set.
    fn check(&self) -> Result<(), Error> {
        if cfg!(debug_assertions) {
            let gl = &self.context;
            let err = Error::from_error_code(unsafe { gl.GetError() });
            if err != Error::NoError {
                return Err(err)
            }
        }
        Ok(())
    }
}

/// Single-threaded `Arc`.
/// Wrapper for `Arc` that allows you to `Send` it even if `T: !Sync`.
/// Yet internal data cannot be accessed outside of the thread where it was created.
pub struct Starc<T: ?Sized> {
    arc: Arc<T>,
    thread: ThreadId,
}

impl<T: ?Sized> Clone for Starc<T> {
    fn clone(&self) -> Self {
        Self {
            arc: self.arc.clone(),
            thread: self.thread,
        }
    }
}

impl<T: ?Sized> fmt::Debug for Starc<T> {
     fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
         write!(fmt, "{:p}@{:?}", self.arc, self.thread)
     }
}

impl<T> Starc<T> {
    fn new(value: T) -> Self {
        Starc {
            arc: Arc::new(value),
            thread: thread::current().id(),
        }
    }
}

unsafe impl<T: ?Sized> Send for Starc<T> {}
unsafe impl<T: ?Sized> Sync for Starc<T> {}

impl<T: ?Sized> Deref for Starc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        assert_eq!(thread::current().id(), self.thread);
        &*self.arc
    }
}

#[derive(Debug)]
pub struct PhysicalDevice(Starc<Share>);

impl PhysicalDevice {
    fn new_adapter<F>(fn_proc: F) -> hal::Adapter<Backend>
    where F: FnMut(&str) -> *const std::os::raw::c_void
    {
        let gl = gl::Gl::load_with(fn_proc);
        // query information
        let (info, features, legacy_features, limits, private_caps) = info::query_all(&gl);
        info!("Vendor: {:?}", info.platform_name.vendor);
        info!("Renderer: {:?}", info.platform_name.renderer);
        info!("Version: {:?}", info.version);
        info!("Shading Language: {:?}", info.shading_language);
        info!("Features: {:?}", features);
        info!("Legacy Features: {:?}", legacy_features);
        debug!("Loaded Extensions:");
        for extension in info.extensions.iter() {
            debug!("- {}", *extension);
        }
        let name = info.platform_name.renderer.into();

        // create the shared context
        let share = Share {
            context: gl,
            info,
            features,
            legacy_features,
            limits,
            private_caps,
            open: Cell::new(false),
        };
        if let Err(err) = share.check() {
            panic!("Error querying info: {:?}", err);
        }

        hal::Adapter {
            info: hal::AdapterInfo {
                name,
                vendor: 0, // TODO
                device: 0, // TODO
                software_rendering: false, // not always true ..
            },
            physical_device: PhysicalDevice(Starc::new(share)),
            queue_families: vec![QueueFamily],
        }
    }

    /// Get GL-specific legacy feature flags.
    pub fn legacy_features(&self) -> &info::LegacyFeatures {
        &self.0.legacy_features
    }
}

impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(
        &self, families: Vec<(&QueueFamily, Vec<hal::QueuePriority>)>,
    ) -> Result<hal::Gpu<Backend>, error::DeviceCreationError> {
        // Can't have multiple logical devices at the same time
        // as they would share the same context.
        if self.0.open.get() {
            return Err(error::DeviceCreationError::TooManyObjects);
        }
        self.0.open.set(true);

        // initialize permanent states
        let gl = &self.0.context;
        if self.0.legacy_features.contains(info::LegacyFeatures::SRGB_COLOR) {
            unsafe {
                gl.Enable(gl::FRAMEBUFFER_SRGB);
            }
        }
        unsafe {
            gl.PixelStorei(gl::UNPACK_ALIGNMENT, 1);

            if !self.0.info.version.is_embedded {
                gl.Enable(gl::PROGRAM_POINT_SIZE);
            }
        }

        // create main VAO and bind it
        let mut vao = 0;
        if self.0.private_caps.vertex_array {
            unsafe {
                gl.GenVertexArrays(1, &mut vao);
                gl.BindVertexArray(vao);
            }
        }

        if let Err(err) = self.0.check() {
            panic!("Error opening adapter: {:?}", err);
        }

        Ok(hal::Gpu {
            device: Device::new(self.0.clone()),
            queues: Queues::new(families
                .into_iter()
                .map(|(proto_family, priorities)| {
                    assert_eq!(priorities.len(), 1);
                    let mut family = hal::backend::RawQueueGroup::new(*proto_family);
                    let queue = queue::CommandQueue::new(&self.0, vao);
                    family.add_queue(queue);
                    (QueueFamilyId(0), family)
                })
                .collect()),
        })
    }

    fn format_properties(&self, _: Option<hal::format::Format>) -> hal::format::Properties {
        unimplemented!()
    }

    fn image_format_properties(
        &self, _format: hal::format::Format, _dimensions: u8, _tiling: image::Tiling,
        _usage: image::Usage, _storage_flags: image::StorageFlags,
    ) -> Option<image::FormatProperties> {
        None //TODO
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        use hal::memory::Properties;

        // COHERENT flags require that the backend does flushing and invalidation
        // by itself. If we move towards persistent mapping we need to re-evaluate it.
        let memory_types = if self.0.private_caps.map {
            vec![
                hal::MemoryType {
                    properties: Properties::DEVICE_LOCAL,
                    heap_index: 1,
                },
                hal::MemoryType { // upload
                    properties: Properties::CPU_VISIBLE | Properties::COHERENT,
                    heap_index: 0,
                },
                hal::MemoryType { // download
                    properties: Properties::CPU_VISIBLE | Properties::COHERENT | Properties::CPU_CACHED,
                    heap_index: 0,
                },
            ]
        } else {
            vec![
                hal::MemoryType {
                    properties: Properties::DEVICE_LOCAL,
                    heap_index: 0,
                },
            ]
        };

        hal::MemoryProperties {
            memory_types,
            memory_heaps: vec![!0, !0],
        }
    }

    fn features(&self) -> hal::Features {
        self.0.features
    }

    fn limits(&self) -> hal::Limits {
        self.0.limits
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QueueFamily;

impl hal::QueueFamily for QueueFamily {
    fn queue_type(&self) -> hal::QueueType { hal::QueueType::General }
    fn max_queues(&self) -> usize { 1 }
    fn id(&self) -> QueueFamilyId { QueueFamilyId(0) }
}
