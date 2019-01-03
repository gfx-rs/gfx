//! OpenGL implementation of a device, striving to support OpenGL 2.0 with at
//! least VAOs, but using newer extensions when available.

#![allow(missing_docs, missing_copy_implementations)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
extern crate gfx_gl as gl;
extern crate gfx_hal as hal;
#[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
pub extern crate glutin;
extern crate smallvec;
#[cfg(not(target_arch = "wasm32"))]
extern crate spirv_cross;
#[cfg(target_arch = "wasm32")]
extern crate wasm_bindgen;
extern crate glow;

use std::cell::Cell;
use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::thread::{self, ThreadId};

use crate::hal::queue::{QueueFamilyId, Queues};
use crate::hal::{error, image, pso};

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

#[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
pub use crate::window::glutin::{config_context, Headless, Surface, Swapchain};
#[cfg(target_arch = "wasm32")]
pub use crate::window::web::{Surface, Swapchain, Window};

use glow::Context;
#[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
pub use glow::native::Context as GlContext;
#[cfg(target_arch = "wasm32")]
pub use glow::web::Context as GlContext;

pub(crate) struct GlContainer {
    context: GlContext,
}

impl GlContainer {
    fn make_current(&self) {
        // Unimplemented
    }
}

impl Deref for GlContainer {
    type Target = GlContext;
    fn deref(&self) -> &GlContext {
        #[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
        self.make_current();
        &self.context
    }
}

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
    type Framebuffer = Option<native::FrameBuffer>;

    type Buffer = native::Buffer;
    type BufferView = native::BufferView;
    type Image = native::Image;
    type ImageView = native::ImageView;
    type Sampler = native::FatSampler;

    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineLayout = native::PipelineLayout;
    type PipelineCache = ();
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
            gl::NO_ERROR => Error::NoError,
            gl::INVALID_ENUM => Error::InvalidEnum,
            gl::INVALID_VALUE => Error::InvalidValue,
            gl::INVALID_OPERATION => Error::InvalidOperation,
            gl::INVALID_FRAMEBUFFER_OPERATION => Error::InvalidFramebufferOperation,
            gl::OUT_OF_MEMORY => Error::OutOfMemory,
            _ => Error::UnknownError,
        }
    }
}

/// Internal struct of shared data between the physical and logical device.
struct Share {
    context: GlContainer,
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
            let err = Error::from_error_code(unsafe { gl.get_error() });
            if err != Error::NoError {
                return Err(err);
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
    #[inline]
    fn new(value: T) -> Self {
        Starc {
            arc: Arc::new(value),
            thread: thread::current().id(),
        }
    }

    #[inline]
    pub fn try_unwrap(self) -> Result<T, Self> {
        let a = Arc::try_unwrap(self.arc);
        let thread = self.thread;
        a.map_err(|a| Starc {
            arc: a,
            thread: thread,
        })
    }

    #[inline]
    pub fn downgrade(this: &Starc<T>) -> Wstarc<T> {
        Wstarc {
            weak: Arc::downgrade(&this.arc),
            thread: this.thread,
        }
    }

    #[inline]
    pub fn get_mut(this: &mut Starc<T>) -> Option<&mut T> {
        Arc::get_mut(&mut this.arc)
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

/// Single-threaded `Weak`.
/// Wrapper for `Weak` that allows you to `Send` it even if `T: !Sync`.
/// Yet internal data cannot be accessed outside of the thread where it was created.
pub struct Wstarc<T: ?Sized> {
    weak: Weak<T>,
    thread: ThreadId,
}
impl<T> Wstarc<T> {
    pub fn upgrade(&self) -> Option<Starc<T>> {
        let thread = self.thread;
        self.weak.upgrade().map(|arc| Starc { arc, thread })
    }
}
unsafe impl<T: ?Sized> Send for Wstarc<T> {}
unsafe impl<T: ?Sized> Sync for Wstarc<T> {}

#[derive(Debug)]
pub struct PhysicalDevice(Starc<Share>);

impl PhysicalDevice {
    #[allow(unused)]
    fn new_adapter<F>(fn_proc: F, webgl_context_id: Option<&str>) -> hal::Adapter<Backend>
    where
        F: FnMut(&str) -> *const std::os::raw::c_void,
    {
        #[cfg(target_arch = "wasm32")]
        let context = {
            use wasm_bindgen::JsCast;
            let canvas = web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id(webgl_context_id.unwrap())
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();
            let webgl2_context = canvas
                .get_context("webgl2")
                .unwrap()
                .unwrap()
                .dyn_into::<web_sys::WebGl2RenderingContext>()
                .unwrap();
            glow::web::Context::from_webgl2_context(webgl2_context)
        };

        #[cfg(not(target_arch = "wasm32"))]
        let context = glow::native::Context::from_loader_function(fn_proc);

        let gl = GlContainer {
            context,
        };

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
        let vendor: std::string::String = info.platform_name.vendor.into();
        let renderer: std::string::String = info.platform_name.renderer.into();

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

        // opengl has no way to discern device_type, so we can try to infer it from the renderer string
        let vendor_lower = vendor.to_lowercase();
        let renderer_lower = renderer.to_lowercase();
        let strings_that_imply_integrated = [
            " xpress", // space here is on purpose so we don't match express
            "radeon hd 4200",
            "radeon hd 4250",
            "radeon hd 4290",
            "radeon hd 4270",
            "radeon hd 4225",
            "radeon hd 3100",
            "radeon hd 3200",
            "radeon hd 3000",
            "radeon hd 3300",
            "radeon(tm) r4 graphics",
            "radeon(tm) r5 graphics",
            "radeon(tm) r6 graphics",
            "radeon(tm) r7 graphics",
            "radeon r7 graphics",
            "nforce", // all nvidia nforce are integrated
            "tegra",  // all nvidia tegra are integrated
            "shield", // all nvidia shield are integrated
            "igp",
            "mali",
            "intel",
        ];
        // todo: Intel will release a discrete gpu soon, and we will need to update this logic when they do
        let inferred_device_type = if vendor_lower.contains("qualcomm")
            || vendor_lower.contains("intel")
            || strings_that_imply_integrated
                .into_iter()
                .any(|&s| renderer_lower.contains(s))
        {
            hal::adapter::DeviceType::IntegratedGpu
        } else {
            hal::adapter::DeviceType::DiscreteGpu
        };

        // source: Sascha Willems at Vulkan
        let vendor_id = if vendor_lower.contains("amd") {
            0x1002
        } else if vendor_lower.contains("imgtec") {
            0x1010
        } else if vendor_lower.contains("nvidia") {
            0x10DE
        } else if vendor_lower.contains("arm") {
            0x13B5
        } else if vendor_lower.contains("qualcomm") {
            0x5143
        } else if vendor_lower.contains("intel") {
            0x8086
        } else {
            0
        };

        hal::Adapter {
            info: hal::AdapterInfo {
                name,
                vendor: vendor_id,
                device: 0,
                device_type: inferred_device_type,
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
    unsafe fn open(
        &self,
        families: &[(&QueueFamily, &[hal::QueuePriority])],
        requested_features: hal::Features,
    ) -> Result<hal::Gpu<Backend>, error::DeviceCreationError> {
        // Can't have multiple logical devices at the same time
        // as they would share the same context.
        if self.0.open.get() {
            return Err(error::DeviceCreationError::TooManyObjects);
        }
        self.0.open.set(true);

        // TODO: Check for support in the LeagcyFeatures struct too
        if !self.features().contains(requested_features) {
            return Err(error::DeviceCreationError::MissingFeature);
        }

        // initialize permanent states
        let gl = &self.0.context;
        if self
            .0
            .legacy_features
            .contains(info::LegacyFeatures::SRGB_COLOR)
        {
            // TODO: Find way to emulate this on older Opengl versions.
            gl.enable(glow::FRAMEBUFFER_SRGB);
        }
        unsafe {
            gl.pixel_store_i32(glow::PixelStoreI32Parameter::UnpackAlignment, 1);
        }

        // create main VAO and bind it
        let mut vao = None;
        if self.0.private_caps.vertex_array {
            vao = Some(gl.create_vertex_array().unwrap());
            gl.bind_vertex_array(vao);
        }

        if let Err(err) = self.0.check() {
            panic!("Error opening adapter: {:?}", err);
        }

        Ok(hal::Gpu {
            device: Device::new(self.0.clone()),
            queues: Queues::new(
                families
                    .into_iter()
                    .map(|&(proto_family, priorities)| {
                        assert_eq!(priorities.len(), 1);
                        let mut family = hal::backend::RawQueueGroup::new(proto_family.clone());
                        let queue = queue::CommandQueue::new(&self.0, vao);
                        family.add_queue(queue);
                        family
                    })
                    .collect(),
            ),
        })
    }

    fn format_properties(&self, _: Option<hal::format::Format>) -> hal::format::Properties {
        unimplemented!()
    }

    fn image_format_properties(
        &self,
        _format: hal::format::Format,
        _dimensions: u8,
        _tiling: image::Tiling,
        _usage: image::Usage,
        _view_caps: image::ViewCapabilities,
    ) -> Option<image::FormatProperties> {
        None //TODO
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        use crate::hal::memory::Properties;

        // COHERENT flags require that the backend does flushing and invalidation
        // by itself. If we move towards persistent mapping we need to re-evaluate it.
        let memory_types = if self.0.private_caps.map {
            vec![
                hal::MemoryType {
                    properties: Properties::DEVICE_LOCAL,
                    heap_index: 1,
                },
                hal::MemoryType {
                    // upload
                    properties: Properties::CPU_VISIBLE | Properties::COHERENT,
                    heap_index: 0,
                },
                hal::MemoryType {
                    // download
                    properties: Properties::CPU_VISIBLE
                        | Properties::COHERENT
                        | Properties::CPU_CACHED,
                    heap_index: 0,
                },
            ]
        } else {
            vec![hal::MemoryType {
                properties: Properties::DEVICE_LOCAL,
                heap_index: 0,
            }]
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
    fn queue_type(&self) -> hal::QueueType {
        hal::QueueType::General
    }
    fn max_queues(&self) -> usize {
        1
    }
    fn id(&self) -> QueueFamilyId {
        QueueFamilyId(0)
    }
}
