/*!
# OpenGL backend internals.

The main target is OpenGL ES3 on Linux and Android.

## Binding model

Binding model is the biggest and most painful difference with gfx-hal API.

First, the bindings get linearized into the pipeline layout (see `PipelineLayoutSet`).
Then a pipeline gets created, and we track all the texture-sampler associations.
We only support at most one sampler used with each texture so far. The linear index
of this sampler is stored per texture slot in `SamplerBindMap` array.

The texture-sampler pairs get potentially invalidated in 2 places:
  - when a new pipeline is bound, we update the linear indices of associated samplers
  - when a new descriptor set is bound, we update both the textures and the samplers

We expect that the changes to sampler states between any 2 pipelines of the same layout
will be minimal, if any.
*/

#![allow(missing_docs, missing_copy_implementations)]

#[macro_use]
extern crate log;

use std::{
    cell::Cell,
    fmt,
    ops::{Deref, Range},
    sync::{Arc, Weak},
    thread,
};

use hal::{adapter, buffer, image, memory, queue as q};

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

// Web implementation
#[cfg(target_arch = "wasm32")]
pub use window::web::{Instance, Surface, Swapchain};

#[cfg(not(target_arch = "wasm32"))]
pub use window::egl::{Instance, Surface, Swapchain};

pub use glow::Context as GlContext;
use glow::HasContext;

type ColorSlot = u8;

// we can support more samplers if not every one of them is used at a time,
// but it probably doesn't worth it.
const MAX_SAMPLERS: usize = 16;
//TODO: has to be within glow::MAX_COMBINED_TEXTURE_IMAGE_UNITS
const MAX_TEXTURE_SLOTS: usize = 16;
const MAX_COLOR_ATTACHMENTS: usize = 16;

struct GlContainer {
    context: GlContext,
}

impl Deref for GlContainer {
    type Target = GlContext;
    fn deref(&self) -> &GlContext {
        &self.context
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}

impl hal::Backend for Backend {
    type Instance = Instance;

    type PhysicalDevice = PhysicalDevice;
    type Device = Device;
    type Surface = Surface;

    type QueueFamily = QueueFamily;
    type CommandQueue = queue::CommandQueue;
    type CommandBuffer = command::CommandBuffer;

    type Memory = native::Memory;
    type CommandPool = pool::CommandPool;

    type ShaderModule = native::ShaderModule;
    type RenderPass = native::RenderPass;
    type Framebuffer = native::Framebuffer;

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

fn debug_message_callback(source: u32, gltype: u32, id: u32, severity: u32, message: &str) {
    let source_str = match source {
        glow::DEBUG_SOURCE_API => "API",
        glow::DEBUG_SOURCE_WINDOW_SYSTEM => "Window System",
        glow::DEBUG_SOURCE_SHADER_COMPILER => "ShaderCompiler",
        glow::DEBUG_SOURCE_THIRD_PARTY => "Third Party",
        glow::DEBUG_SOURCE_APPLICATION => "Application",
        glow::DEBUG_SOURCE_OTHER => "Other",
        _ => unreachable!(),
    };

    let log_severity = match severity {
        glow::DEBUG_SEVERITY_HIGH => log::Level::Error,
        glow::DEBUG_SEVERITY_MEDIUM => log::Level::Warn,
        glow::DEBUG_SEVERITY_LOW => log::Level::Info,
        glow::DEBUG_SEVERITY_NOTIFICATION => log::Level::Trace,
        _ => unreachable!(),
    };

    let type_str = match gltype {
        glow::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "Deprecated Behavior",
        glow::DEBUG_TYPE_ERROR => "Error",
        glow::DEBUG_TYPE_MARKER => "Marker",
        glow::DEBUG_TYPE_OTHER => "Other",
        glow::DEBUG_TYPE_PERFORMANCE => "Performance",
        glow::DEBUG_TYPE_POP_GROUP => "Pop Group",
        glow::DEBUG_TYPE_PORTABILITY => "Portability",
        glow::DEBUG_TYPE_PUSH_GROUP => "Push Group",
        glow::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "Undefined Behavior",
        _ => unreachable!(),
    };

    log!(
        log_severity,
        "[{}/{}] ID {} : {}",
        source_str,
        type_str,
        id,
        message
    );
}

const DEVICE_LOCAL_HEAP: usize = 0;
const CPU_VISIBLE_HEAP: usize = 1;

/// Memory types in the OpenGL backend are either usable for buffers and are backed by a real OpenGL
/// buffer, or are used for images and are fake and not backed by any real raw buffer.
#[derive(Copy, Clone, Debug)]
enum MemoryUsage {
    Buffer(buffer::Usage),
    Image,
}

/// Internal struct of shared data between the physical and logical device.
struct Share {
    context: GlContainer,
    info: Info,
    supported_features: hal::Features,
    legacy_features: info::LegacyFeatures,
    limits: hal::Limits,
    public_caps: hal::Capabilities,
    private_caps: info::PrivateCaps,
    // Indicates if there is an active logical device.
    open: Cell<bool>,
    memory_types: Vec<(adapter::MemoryType, MemoryUsage)>,
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

    fn buffer_memory_type_mask(&self, usage: buffer::Usage) -> u32 {
        let mut type_mask = 0;
        for (type_index, &(_, kind)) in self.memory_types.iter().enumerate() {
            match kind {
                MemoryUsage::Buffer(buffer_usage) => {
                    if buffer_usage.contains(usage) {
                        type_mask |= 1 << type_index;
                    }
                }
                MemoryUsage::Image => {}
            }
        }
        if type_mask == 0 {
            error!(
                "gl backend capability does not allow a buffer with usage {:?}",
                usage
            );
        }
        type_mask
    }

    fn image_memory_type_mask(&self) -> u32 {
        let mut type_mask = 0;
        for (type_index, &(_, kind)) in self.memory_types.iter().enumerate() {
            match kind {
                MemoryUsage::Buffer(_) => {}
                MemoryUsage::Image => {
                    type_mask |= 1 << type_index;
                }
            }
        }
        assert_ne!(type_mask, 0);
        type_mask
    }
}

/// Single-threaded `Arc`.
/// Wrapper for `Arc` that allows you to `Send` it even if `T: !Sync`.
/// Yet internal data cannot be accessed outside of the thread where it was created.
pub struct Starc<T: ?Sized> {
    arc: Arc<T>,
    thread: thread::ThreadId,
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
}

impl<T> Starc<T>
where
    T: ?Sized,
{
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
    thread: thread::ThreadId,
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
    fn new_adapter(context: GlContext) -> adapter::Adapter<Backend> {
        let gl = GlContainer { context };
        // query information
        let (info, supported_features, legacy_features, limits, public_caps, private_caps) =
            info::query_all(&gl);
        info!("Vendor: {:?}", info.platform_name.vendor);
        info!("Renderer: {:?}", info.platform_name.renderer);
        info!("Version: {:?}", info.version);
        info!("Shading Language: {:?}", info.shading_language);
        info!("Supported Features: {:?}", supported_features);
        info!("Legacy Features: {:?}", legacy_features);
        debug!("Public capabilities: {:#?}", public_caps);
        debug!("Private capabilities: {:#?}", private_caps);
        debug!("Loaded Extensions:");
        for extension in info.extensions.iter() {
            debug!("- {}", *extension);
        }
        let name = info.platform_name.renderer.clone();
        let vendor: std::string::String = info.platform_name.vendor.clone();
        let renderer: std::string::String = info.platform_name.renderer.clone();

        let mut memory_types = Vec::new();

        let mut add_buffer_memory_type = |memory_type: adapter::MemoryType| {
            if private_caps.index_buffer_role_change {
                // If `index_buffer_role_change` is true, we can use a buffer for any role
                memory_types.push((memory_type, MemoryUsage::Buffer(buffer::Usage::all())));
            } else {
                // If `index_buffer_role_change` is false, ELEMENT_ARRAY_BUFFER buffers may not be
                // mixed with other targets, so we need to provide one type of memory for INDEX
                // usage only and another type for all other uses.
                memory_types.push((memory_type, MemoryUsage::Buffer(buffer::Usage::INDEX)));
                memory_types.push((
                    memory_type,
                    MemoryUsage::Buffer(buffer::Usage::all() - buffer::Usage::INDEX),
                ));
            }
        };

        // Coherent memory is only available if we have `glBufferStorage`
        let coherent_flag = if private_caps.buffer_storage {
            memory::Properties::COHERENT
        } else {
            memory::Properties::empty()
        };
        // Mimicking vulkan, memory types with more flags should come before those with fewer flags
        add_buffer_memory_type(adapter::MemoryType {
            properties: coherent_flag
                | memory::Properties::CPU_VISIBLE
                | memory::Properties::CPU_CACHED,
            heap_index: CPU_VISIBLE_HEAP,
        });
        add_buffer_memory_type(adapter::MemoryType {
            properties: coherent_flag | memory::Properties::CPU_VISIBLE,
            heap_index: CPU_VISIBLE_HEAP,
        });

        add_buffer_memory_type(adapter::MemoryType {
            properties: memory::Properties::DEVICE_LOCAL,
            heap_index: DEVICE_LOCAL_HEAP,
        });

        // There is always a single device-local memory type for images
        memory_types.push((
            adapter::MemoryType {
                properties: memory::Properties::DEVICE_LOCAL,
                heap_index: DEVICE_LOCAL_HEAP,
            },
            MemoryUsage::Image,
        ));

        assert!(memory_types.len() <= 64);

        log::info!("Memory types: {:#?}", memory_types);

        // create the shared context
        let share = Share {
            context: gl,
            info,
            supported_features,
            legacy_features,
            limits,
            public_caps,
            private_caps,
            open: Cell::new(false),
            memory_types,
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
        let strings_that_imply_cpu = ["mesa offscreen", "swiftshader"];
        // todo: Intel will release a discrete gpu soon, and we will need to update this logic when they do
        let inferred_device_type = if vendor_lower.contains("qualcomm")
            || vendor_lower.contains("intel")
            || strings_that_imply_integrated
                .iter()
                .any(|&s| renderer_lower.contains(s))
        {
            hal::adapter::DeviceType::IntegratedGpu
        } else if strings_that_imply_cpu
            .iter()
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

        adapter::Adapter {
            info: adapter::AdapterInfo {
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

impl adapter::PhysicalDevice<Backend> for PhysicalDevice {
    unsafe fn open(
        &self,
        families: &[(&QueueFamily, &[q::QueuePriority])],
        requested_features: hal::Features,
    ) -> Result<adapter::Gpu<Backend>, hal::device::CreationError> {
        // Can't have multiple logical devices at the same time
        // as they would share the same context.
        if self.0.open.get() {
            return Err(hal::device::CreationError::TooManyObjects);
        }
        self.0.open.set(true);

        // TODO: Check for support in the LegacyFeatures struct too
        if !self.features().contains(requested_features) {
            return Err(hal::device::CreationError::MissingFeature);
        }

        // initialize permanent states
        let gl = &self.0.context;

        if cfg!(debug_assertions) && !cfg!(target_arch = "wasm32") && gl.supports_debug() {
            gl.enable(glow::DEBUG_OUTPUT);
            gl.debug_message_callback(debug_message_callback);
        }

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

        Ok(adapter::Gpu {
            device: Device::new(self.0.clone(), requested_features),
            queue_groups: families
                .into_iter()
                .map(|&(_family, priorities)| {
                    assert_eq!(priorities.len(), 1);
                    let mut family = q::QueueGroup::new(q::QueueFamilyId(0));
                    let queue = queue::CommandQueue::new(&self.0, requested_features, vao);
                    family.add_queue(queue);
                    family
                })
                .collect(),
        })
    }

    fn format_properties(&self, _: Option<hal::format::Format>) -> hal::format::Properties {
        use hal::format::{BufferFeature as Bf, ImageFeature as If};

        // TODO: These are for show
        hal::format::Properties {
            linear_tiling: If::TRANSFER_SRC | If::TRANSFER_DST | If::empty(),
            optimal_tiling: If::TRANSFER_SRC | If::TRANSFER_DST | If::SAMPLED,
            buffer_features: Bf::VERTEX,
        }
    }

    fn image_format_properties(
        &self,
        format: hal::format::Format,
        _dimensions: u8,
        _tiling: image::Tiling,
        _usage: image::Usage,
        _view_caps: image::ViewCapabilities,
    ) -> Option<image::FormatProperties> {
        conv::describe_format(format)?;
        Some(image::FormatProperties {
            max_extent: image::Extent {
                width: !0,
                height: !0,
                depth: !0,
            },
            max_levels: !0,
            max_layers: !0,
            sample_count_mask: 127,
            max_resource_size: !0,
        })
    }

    fn memory_properties(&self) -> adapter::MemoryProperties {
        adapter::MemoryProperties {
            memory_types: self
                .0
                .memory_types
                .iter()
                .map(|(mem_type, _)| *mem_type)
                .collect(),
            memory_heaps: vec![
                adapter::MemoryHeap {
                    size: !0,
                    flags: memory::HeapFlags::DEVICE_LOCAL,
                },
                adapter::MemoryHeap {
                    size: !0,
                    flags: memory::HeapFlags::empty(),
                },
            ],
        }
    }

    fn features(&self) -> hal::Features {
        self.0.supported_features
    }

    fn capabilities(&self) -> hal::Capabilities {
        self.0.public_caps
    }

    fn limits(&self) -> hal::Limits {
        self.0.limits
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QueueFamily;

impl q::QueueFamily for QueueFamily {
    fn queue_type(&self) -> q::QueueType {
        q::QueueType::General
    }
    fn max_queues(&self) -> usize {
        1
    }
    fn id(&self) -> q::QueueFamilyId {
        q::QueueFamilyId(0)
    }
}

fn resolve_sub_range(
    sub: &buffer::SubRange,
    whole: Range<buffer::Offset>,
) -> Range<buffer::Offset> {
    let end = sub.size.map_or(whole.end, |s| whole.start + sub.offset + s);
    whole.start + sub.offset..end
}
