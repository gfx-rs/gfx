//! OpenGL implementation of a device, striving to support OpenGL 2.0 with at
//! least VAOs, but using newer extensions when available.

#![allow(missing_docs, missing_copy_implementations)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
extern crate gfx_hal as hal;
#[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
pub extern crate glutin;

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

#[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
pub use glow::native::Context as GlContext;
#[cfg(target_arch = "wasm32")]
pub use glow::web::Context as GlContext;
use glow::Context;

pub(crate) const IMAGE_MEMORY_TYPE: usize = 0;
pub(crate) const INDEX_MEMORY_TYPE: usize = 2;
pub(crate) const IMAGE_MEM_TYPE_MASK: u64 = 0x1;
pub(crate) const OTHER_MEM_TYPE_MASK: u64 = 0x2;
pub(crate) const INDEX_MEM_TYPE_MASK: u64 = 0x4;

pub(crate) struct GlContainer {
    context: GlContext,
}

impl GlContainer {
    fn make_current(&self) {
        // Unimplemented
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn from_fn_proc<F>(fn_proc: F) -> GlContainer
    where F: FnMut(&str) -> *const std::os::raw::c_void {
        let context = glow::native::Context::from_loader_function(fn_proc);
        GlContainer { context }
    }

    #[cfg(target_arch = "wasm32")]
    fn from_new_canvas() -> GlContainer {
        let context = {
            use wasm_bindgen::JsCast;
            let document = web_sys::window()
                .and_then(|win| win.document())
                .expect("Cannot get document");
            let canvas = document
                .create_element("canvas")
                .expect("Cannot create canvas")
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("Cannot get canvas element");
            // TODO: Remove hardcoded width/height
            canvas.set_attribute("width", "640").expect("Cannot set width");
            canvas.set_attribute("height", "480").expect("Cannot set height");
            let context_options = js_sys::Object::new();
            js_sys::Reflect::set(
                &context_options,
                &"antialias".into(),
                &wasm_bindgen::JsValue::FALSE
            ).expect("Cannot create context options");
            let webgl2_context = canvas
                .get_context_with_context_options("webgl2", &context_options)
                .expect("Cannot create WebGL2 context")
                .and_then(|context| context.dyn_into::<web_sys::WebGl2RenderingContext>().ok())
                .expect("Cannot convert into WebGL2 context");
            document.body()
                .expect("Cannot get document body")
                .append_child(&canvas)
                .expect("Cannot insert canvas into document body");
            glow::web::Context::from_webgl2_context(webgl2_context)
        };
        GlContainer { context }
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
    type Event = ();
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
    pub fn from_error_code(error_code: u32) -> Error {
        match error_code {
            glow::NO_ERROR => Error::NoError,
            glow::INVALID_ENUM => Error::InvalidEnum,
            glow::INVALID_VALUE => Error::InvalidValue,
            glow::INVALID_OPERATION => Error::InvalidOperation,
            glow::INVALID_FRAMEBUFFER_OPERATION => Error::InvalidFramebufferOperation,
            glow::OUT_OF_MEMORY => Error::OutOfMemory,
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
    fn new_adapter(gl: GlContainer) -> hal::Adapter<Backend> {
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
        let name = info.platform_name.renderer.clone();
        let vendor: std::string::String = info.platform_name.vendor.clone();
        let renderer: std::string::String = info.platform_name.renderer.clone();

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
        let strings_that_imply_cpu = [
            "mesa offscreen"
        ];
        // todo: Intel will release a discrete gpu soon, and we will need to update this logic when they do
        let inferred_device_type = if vendor_lower.contains("qualcomm")
            || vendor_lower.contains("intel")
            || strings_that_imply_integrated
                .into_iter()
                .any(|&s| renderer_lower.contains(s))
        {
            hal::adapter::DeviceType::IntegratedGpu
        } else if strings_that_imply_cpu
            .into_iter()
            .any(|&s| renderer_lower.contains(s))
        {
            hal::adapter::DeviceType::Cpu
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

        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

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
        use hal::format::ImageFeature;
        use hal::format::BufferFeature;

        // TODO: These are for show
        hal::format::Properties {
            linear_tiling: ImageFeature::SAMPLED,
            optimal_tiling: ImageFeature::SAMPLED,
            buffer_features: BufferFeature::VERTEX,
        }
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
        let caps = &self.0.private_caps;
        assert!(caps.map || caps.emulate_map);
        let memory_types = vec![
            // Faked DEVICE_LOCAL memory for images, no gl buffer is actually allocated for it.
            hal::MemoryType {
                properties: Properties::DEVICE_LOCAL,
                heap_index: 0,
            },
            // Memory type for uses other than images and INDEX
            hal::MemoryType {
                properties: Properties::CPU_VISIBLE
                    | Properties::COHERENT
                    | Properties::CPU_CACHED,
                heap_index: 1,
            },
            // For security reasons, WebGL does not allow "element array buffers" to be used as any
            // other kind of buffer.  We need to provide a unique type of memory specifically for
            // buffers with INDEX usage.
            hal::MemoryType {
                properties: Properties::CPU_VISIBLE
                    | Properties::COHERENT
                    | Properties::CPU_CACHED,
                heap_index: 2,
            },
        ];

        hal::MemoryProperties {
            memory_types,
            memory_heaps: vec![!0, !0, !0],
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

#[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
pub enum Instance {
    Headless(Headless),
    Surface(Surface)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
impl hal::Instance for Instance {
    type Backend = Backend;
    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        match self {
            Instance::Headless(instance) => instance.enumerate_adapters(),
            Instance::Surface(instance) => instance.enumerate_adapters(),
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "glutin"))]
impl Instance {
    /// TODO: Update portability to make this more flexible
    #[cfg(target_os = "linux")]
    pub fn create(_: &str, _: u32) -> Instance {
        use glutin::os::unix::OsMesaContextExt;
        use glutin::ContextTrait;
        let size = glutin::dpi::PhysicalSize::from((800, 600));
        let builder = glutin::ContextBuilder::new()
            .with_hardware_acceleration(Some(false));
        let context: glutin::Context = OsMesaContextExt::new_osmesa(builder, size)
            .expect("failed to create osmesa context");
        unsafe {
            context.make_current()
                .expect("failed to make context current");
        }
        let headless = Headless(context);
        Instance::Headless(headless)
    }
}
