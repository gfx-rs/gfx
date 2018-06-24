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

#[cfg(feature = "glutin")]
use glutin::GlContext;

use std::cell::Cell;
use std::fmt;
use std::sync::{Arc, Mutex, Weak};
use std::ops::Deref;
use std::thread::{self, ThreadId};

use hal::{error, image, pso};
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
pub use window::glutin::{Surface, Swapchain, Window};

#[cfg(feature = "glutin")]
pub(crate) struct InstanceContext {
    el: Arc<Mutex<glutin::EventsLoop>>,
    context: glutin::Context,
}

#[cfg(not(feature = "glutin"))]
pub struct Instance {
    physical_device: PhysicalDevice,
    name: String,
    device: Arc<Device>,
    vao: u32,
}

#[cfg(feature = "glutin")]
pub struct Instance {
    instance_context: Starc<InstanceContext>,
    physical_device: PhysicalDevice,
    name: String,
    device: Arc<Device>,
    vao: u32,
}

#[cfg(not(feature = "glutin"))]
pub(crate) enum Contexts {
    Instance(Wstarc<Instance>),
}

#[cfg(feature = "glutin")]
pub(crate) enum Contexts {
    InstanceContext(Wstarc<InstanceContext>),
    Instance(Wstarc<Instance>),
    Window(Wstarc<glutin::GlWindow>),
}

#[cfg(feature = "glutin")]
impl Instance {
    pub fn create(_name: &str, _version: u32, el: Arc<Mutex<glutin::EventsLoop>>) -> Starc<Self> {
        let cb = glutin::ContextBuilder::new();
        let context = glutin::Context::new(cb, &el.lock().unwrap(), true).unwrap();

        let instance_context = Starc::new(InstanceContext {
            el,
            context,
        });

        let (name, pd) = PhysicalDevice::new(
            |s| instance_context.context.get_proc_address(s) as *const _,
            Contexts::InstanceContext(Starc::downgrade(&instance_context)),
        );
        let (vao, device) = Instance::new_device(&pd).unwrap();

        let instance = Starc::new(Instance {
            instance_context,
            physical_device: pd,
            name,
            device,
            vao,
        });

        *instance.physical_device.0.context.instance.lock().unwrap()
            = Contexts::Instance(Starc::downgrade(&instance));

        instance
    }

    pub(crate) fn new_device(
        physical_device: &PhysicalDevice,
    ) -> Result<(u32, Arc<Device>), error::DeviceCreationError> {
        // Can't have multiple logical devices at the same time
        // as they would share the same context.
        if physical_device.0.open.get() {
            return Err(error::DeviceCreationError::TooManyObjects);
        }
        physical_device.0.open.set(true);

        // initialize permanent states
        let gl = &physical_device.0.context;
        if physical_device.0.legacy_features.contains(info::LegacyFeatures::SRGB_COLOR) {
            unsafe {
                gl.Enable(gl::FRAMEBUFFER_SRGB);
            }
        }
        unsafe {
            gl.PixelStorei(gl::UNPACK_ALIGNMENT, 1);

            if !physical_device.0.info.version.is_embedded {
                gl.Enable(gl::PROGRAM_POINT_SIZE);
            }
        }

        // create main VAO and bind it
        let mut vao = 0;
        if physical_device.0.private_caps.vertex_array {
            unsafe {
                gl.GenVertexArrays(1, &mut vao);
                gl.BindVertexArray(vao);
            }
        }

        if let Err(err) = physical_device.0.check() {
            panic!("Error opening adapter: {:?}", err);
        }

        Ok((vao, Arc::new(Device::new(physical_device.0.clone()))))
    }
}

#[cfg(feature = "glutin")]
// We can't use `impl Mutex<Instance>` but we can use
// `impl GlInstance for Mutex<Instance>` so we got to use this cheat.
pub trait GlInstance {
    fn create_surface(&self, window: &Arc<Mutex<Window>>) -> Arc<Surface>;
    fn create_window(&self, wb: glutin::WindowBuilder) -> Arc<Mutex<Window>>;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>>;
}

#[cfg(feature = "glutin")]
impl GlInstance for Starc<Instance> {
    fn create_surface(&self, window: &Arc<Mutex<Window>>) -> Arc<Surface> {
        Arc::new(Surface {
            window: Arc::clone(window),
        })
    }

    fn create_window(&self, wb: glutin::WindowBuilder) -> Arc<Mutex<Window>> {
        Window::new(wb)
    }

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        vec![hal::Adapter {
            info: hal::AdapterInfo {
                name: self.name.clone(),
                vendor: 0, // TODO
                device: 0, // TODO
                software_rendering: false, // not always true ..
            },
            physical_device: self.physical_device.clone(),
            queue_families: vec![QueueFamily],
        }]
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl hal::Backend for Backend {
    type PhysicalDevice = PhysicalDevice;
    type Device = Arc<Device>;

    type Surface = Arc<Surface>;
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
            let err = Error::from_error_code(unsafe { gl.GetError() });
            if err != Error::NoError {
                return Err(err)
            }
        }
        Ok(())
    }
}

#[cfg(not(feature = "glutin"))]
pub(crate) struct GlContainer {
    context: gl::Gl,
}

#[cfg(feature = "glutin")]
pub(crate) struct GlContainer {
    context: gl::Gl,
    instance: Mutex<Contexts>,
}

#[cfg(feature = "glutin")]
impl GlContainer {
    fn make_current(&self) {
        match &*self.instance.lock().unwrap() {
            Contexts::InstanceContext(i) => {
                let i = Wstarc::upgrade(i).unwrap();
                if !i.context.is_current() {
                    unsafe { i.context.make_current().unwrap() }
                }
            }
            Contexts::Instance(i) => {
                let i = Wstarc::upgrade(i).unwrap();
                if !i.instance_context.context.is_current() {
                    unsafe { i.instance_context.context.make_current().unwrap() }
                }
            }
            Contexts::Window(i) => {
                let i = Wstarc::upgrade(i).unwrap();
                if !i.context().is_current() {
                    unsafe { i.context().make_current().unwrap() }
                }
            }
        }
    }
}

impl Deref for GlContainer {
    type Target = gl::Gl;
    fn deref(&self) -> &gl::Gl {
        #[cfg(feature = "glutin")]
        self.make_current();
        &self.context
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
    pub fn new(arc: T) -> Self {
        Starc {
            arc: Arc::new(arc),
            thread: thread::current().id(),
        }
    }

    #[inline]
    pub fn try_unwrap(self) -> Result<T, Self> {
        let a = Arc::try_unwrap(self.arc);
        let thread = self.thread;
        a.map_err(|a|
            Starc {
                arc: a,
                thread: thread,
            }
        )
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
        self.weak.upgrade().map(|arc| Starc {
            arc,
            thread,
        })
    }
}

unsafe impl<T: ?Sized> Send for Wstarc<T> {}
unsafe impl<T: ?Sized> Sync for Wstarc<T> {}

#[derive(Debug, Clone)]
pub struct PhysicalDevice(Starc<Share>);

impl PhysicalDevice {
    fn new<F>(fn_proc: F, instance: Contexts) -> (String, PhysicalDevice)
    where F: FnMut(&str) -> *const std::os::raw::c_void
    {
        let gl = GlContainer {
            context: gl::Gl::load_with(fn_proc),
            instance: Mutex::new(instance),
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

        (name, PhysicalDevice(Starc::new(share)))
    }

    /// Get GL-specific legacy feature flags.
    pub fn legacy_features(&self) -> &info::LegacyFeatures {
        &self.0.legacy_features
    }

    fn memory_properties(pcaps: &info::PrivateCaps) -> hal::MemoryProperties {
        use hal::memory::Properties;

        // COHERENT flags require that the backend does flushing and invalidation
        // by itself. If we move towards persistent mapping we need to re-evaluate it.
        let memory_types = if pcaps.map {
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

}

impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(
        &self, families: &[(&QueueFamily, &[hal::QueuePriority])],
    ) -> Result<hal::Gpu<Backend>, error::DeviceCreationError> {
        let instance = match *self.0.context.instance.lock().unwrap() {
            Contexts::Instance(ref i) => Wstarc::upgrade(i).unwrap(),
            _ => panic!(),
        };
        let device = Arc::clone(&instance.device);

        Ok(hal::Gpu {
            device,
            queues: Queues::new(families
                .into_iter()
                .map(|&(proto_family, priorities)| {
                    assert_eq!(priorities.len(), 1);
                    let mut family = hal::backend::RawQueueGroup::new(proto_family.clone());
                    let queue = queue::CommandQueue::new(&self.0, instance.vao);
                    family.add_queue(queue);
                    family
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
        Self::memory_properties(&self.0.private_caps)
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
