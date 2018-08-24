extern crate gfx_hal as hal;
extern crate metal;
#[macro_use] extern crate bitflags;
extern crate cocoa;
extern crate foreign_types;
#[macro_use] extern crate objc;
extern crate core_graphics;
#[macro_use] extern crate log;
extern crate block;
extern crate parking_lot;
extern crate smallvec;
extern crate spirv_cross;
extern crate storage_map;

#[cfg(feature = "winit")]
extern crate winit;
#[cfg(feature = "dispatch")]
extern crate dispatch;

#[path = "../../auxil/range_alloc.rs"]
mod range_alloc;
mod device;
mod window;
mod command;
mod internal;
mod native;
mod conversions;
mod soft;

pub use command::CommandPool;
pub use device::{Device, LanguageVersion, PhysicalDevice};
pub use window::{Surface, Swapchain};

pub type GraphicsCommandPool = CommandPool;

use std::mem;
use std::ptr::NonNull;
use std::os::raw::c_void;
use std::sync::Arc;

use hal::queue::QueueFamilyId;

use core_graphics::base::CGFloat;
use core_graphics::geometry::CGRect;
use objc::runtime::{Class, Object};
use foreign_types::ForeignTypeRef;
use parking_lot::{Condvar, Mutex};


//TODO: investigate why exactly using `u8` here is slower (~5% total).
/// A type representing Metal binding's resource index.
type ResourceIndex = u32;

/// Method of recording one-time-submit command buffers.
#[derive(Clone, Debug, Hash, PartialEq)]
pub enum OnlineRecording {
    /// Record natively on-the-fly.
    Immediate,
    /// Store commands and only start recording at submission time.
    Deferred,
    #[cfg(feature = "dispatch")]
    /// Start recording asynchronously upon finishing each pass.
    Remote(dispatch::QueuePriority),
}

impl Default for OnlineRecording {
    fn default() -> Self {
        OnlineRecording::Immediate
    }
}

const MAX_ACTIVE_COMMAND_BUFFERS: usize = 1 << 14;
const MAX_VISIBILITY_QUERIES: usize = 1 << 14;

#[derive(Debug, Clone, Copy)]
pub struct QueueFamily {}

impl hal::QueueFamily for QueueFamily {
    fn queue_type(&self) -> hal::QueueType { hal::QueueType::General }
    fn max_queues(&self) -> usize { 1 }
    fn id(&self) -> QueueFamilyId { QueueFamilyId(0) }
}

struct VisibilityShared {
    /// Availability buffer is in shared memory, it has N double words for
    /// query results followed by N words for the availability.
    buffer: metal::Buffer,
    allocator: Mutex<range_alloc::RangeAllocator<hal::query::Id>>,
    availability_offset: hal::buffer::Offset,
    condvar: Condvar,
}

struct Shared {
    device: Mutex<metal::Device>,
    queue: Mutex<command::QueueInner>,
    service_pipes: internal::ServicePipes,
    disabilities: PrivateDisabilities,
    visibility: VisibilityShared,
}

unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Shared {
    fn new(device: metal::Device) -> Self {
        let feature_macos_10_14: metal::MTLFeatureSet = unsafe { mem::transmute(10004u64) };
        let visibility = VisibilityShared {
            buffer: device.new_buffer(
                MAX_VISIBILITY_QUERIES as u64 * (mem::size_of::<u64>() + mem::size_of::<u32>()) as u64,
                metal::MTLResourceOptions::StorageModeShared,
            ),
            allocator: Mutex::new(range_alloc::RangeAllocator::new(0 .. MAX_VISIBILITY_QUERIES as hal::query::Id)),
            availability_offset: (MAX_VISIBILITY_QUERIES * mem::size_of::<u64>()) as hal::buffer::Offset,
            condvar: Condvar::new(),
        };
        Shared {
            queue: Mutex::new(command::QueueInner::new(&device, Some(MAX_ACTIVE_COMMAND_BUFFERS))),
            service_pipes: internal::ServicePipes::new(&device),
            disabilities: PrivateDisabilities {
                broken_viewport_near_depth: device.name().starts_with("Intel") &&
                    !device.supports_feature_set(feature_macos_10_14),
            },
            device: Mutex::new(device),
            visibility,
        }
    }
}


pub struct Instance;

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        let mut devices = metal::Device::all();
        devices.sort_by_key(|dev| (dev.is_low_power(), dev.is_headless()));
        devices
            .into_iter()
            .map(|dev| hal::Adapter {
                info: hal::AdapterInfo {
                    name: dev.name().into(),
                    vendor: 0,
                    device: 0,
                    software_rendering: false,
                },
                physical_device: device::PhysicalDevice::new(Arc::new(Shared::new(dev))),
                queue_families: vec![QueueFamily{}],
            })
            .collect()
    }
}

impl Instance {
    pub fn create(_: &str, _: u32) -> Self {
        Instance
    }

    fn create_from_nsview(&self, nsview: *mut c_void) -> window::SurfaceInner {
        unsafe {
            let view: cocoa::base::id = mem::transmute(nsview);
            if view.is_null() {
                panic!("window does not have a valid contentView");
            }

            let class = Class::get("CAMetalLayer").unwrap();
            let render_layer: *mut Object = msg_send![class, new];
            msg_send![view, setLayer: render_layer];
            msg_send![view, retain];
            let bounds: CGRect = msg_send![view, bounds];
            msg_send![render_layer, setBounds: bounds];

            let window: *mut Object = msg_send![view, window];
            if window.is_null() {
                panic!("surface is not attached to a window");
            }
            let scale_factor: CGFloat = msg_send![window, backingScaleFactor];
            msg_send![render_layer, setContentsScale:scale_factor];

            window::SurfaceInner {
                nsview: view,
                render_layer: Mutex::new(render_layer),
            }
        }
    }

    pub fn create_surface_from_nsview(&self, nsview: *mut c_void) -> Surface {
        window::Surface {
            inner: Arc::new(self.create_from_nsview(nsview)),
            has_swapchain: false
        }
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        use winit::os::macos::WindowExt;
        window::Surface {
            inner: Arc::new(self.create_from_nsview(window.get_nsview())),
            has_swapchain: false,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl hal::Backend for Backend {
    type PhysicalDevice = device::PhysicalDevice;
    type Device = device::Device;

    type Surface = window::Surface;
    type Swapchain = window::Swapchain;

    type QueueFamily = QueueFamily;
    type CommandQueue = command::CommandQueue;
    type CommandBuffer = command::CommandBuffer;

    type Memory = native::Memory;
    type CommandPool = command::CommandPool;

    type ShaderModule = native::ShaderModule;
    type RenderPass = native::RenderPass;
    type Framebuffer = native::Framebuffer;

    type UnboundBuffer = native::UnboundBuffer;
    type Buffer = native::Buffer;
    type BufferView = native::BufferView;
    type UnboundImage = native::UnboundImage;
    type Image = native::Image;
    type ImageView = native::ImageView;
    type Sampler = native::Sampler;

    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineCache = native::PipelineCache;
    type PipelineLayout = native::PipelineLayout;
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type DescriptorPool = native::DescriptorPool;
    type DescriptorSet = native::DescriptorSet;

    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
    type QueryPool = native::QueryPool;
}

#[derive(Clone, Debug)]
struct PrivateCapabilities {
    msl_version: metal::MTLLanguageVersion,
    exposed_queues: usize,
    resource_heaps: bool,
    argument_buffers: bool,
    shared_textures: bool,
    base_instance: bool,
    format_depth24_stencil8: bool,
    format_depth32_stencil8_filter: bool,
    format_depth32_stencil8_none: bool,
    format_min_srgb_channels: u8,
    format_b5: bool,
    format_bc: bool,
    format_eac_etc: bool,
    format_astc: bool,
    format_r8unorm_srgb_all: bool,
    format_r8unorm_srgb_no_write: bool,
    format_r8snorm_all: bool,
    format_r16_norm_all: bool,
    format_rg8unorm_srgb_all: bool,
    format_rg8unorm_srgb_no_write: bool,
    format_rg8snorm_all: bool,
    format_r32_all: bool,
    format_r32_no_write: bool,
    format_r32float_no_write_no_filter: bool,
    format_r32float_no_filter: bool,
    format_r32float_all: bool,
    format_rgba8_srgb_all: bool,
    format_rgba8_srgb_no_write: bool,
    format_rgb10a2_unorm_all: bool,
    format_rgb10a2_unorm_no_write: bool,
    format_rgb10a2_uint_color: bool,
    format_rgb10a2_uint_color_write: bool,
    format_rg11b10_all: bool,
    format_rg11b10_no_write: bool,
    format_rgb9e5_all: bool,
    format_rgb9e5_no_write: bool,
    format_rgb9e5_filter_only: bool,
    format_rg32_color: bool,
    format_rg32_color_write: bool,
    format_rg32float_all: bool,
    format_rg32float_color_blend: bool,
    format_rg32float_no_filter: bool,
    format_rgba32int_color: bool,
    format_rgba32int_color_write: bool,
    format_rgba32float_color: bool,
    format_rgba32float_color_write: bool,
    format_rgba32float_all: bool,
    format_depth16unorm: bool,
    format_depth32float_filter: bool,
    format_depth32float_none: bool,
    format_bgr10a2_all: bool,
    format_bgr10a2_no_write: bool,
    max_buffers_per_stage: ResourceIndex,
    max_textures_per_stage: ResourceIndex,
    max_samplers_per_stage: ResourceIndex,
    buffer_alignment: u64,
    max_buffer_size: u64,
    max_texture_size: u64,
}

#[derive(Clone, Copy, Debug)]
struct PrivateDisabilities {
    broken_viewport_near_depth: bool,
}

fn validate_line_width(width: f32) {
    // Note from the Vulkan spec:
    // > If the wide lines feature is not enabled, lineWidth must be 1.0
    // Simply assert and no-op because Metal never exposes `Features::LINE_WIDTH` 
    assert_eq!(width, 1.0);
}


trait AsNative {
    type Native;
    fn from(&Self::Native) -> Self;
    fn as_native(&self) -> &Self::Native;
}

pub type BufferPtr = NonNull<metal::MTLBuffer>;
pub type TexturePtr = NonNull<metal::MTLTexture>;
pub type SamplerPtr = NonNull<metal::MTLSamplerState>;

impl AsNative for BufferPtr {
    type Native = metal::BufferRef;
    #[inline]
    fn from(native: &metal::BufferRef) -> Self {
        unsafe {
            NonNull::new_unchecked(native.as_ptr())
        }
    }
    #[inline]
    fn as_native(&self) -> &metal::BufferRef {
        unsafe {
            metal::BufferRef::from_ptr(self.as_ptr())
        }
    }
}

impl AsNative for TexturePtr {
    type Native = metal::TextureRef;
    #[inline]
    fn from(native: &metal::TextureRef) -> Self {
        unsafe {
            NonNull::new_unchecked(native.as_ptr())
        }
    }
    #[inline]
    fn as_native(&self) -> &metal::TextureRef {
        unsafe {
            metal::TextureRef::from_ptr(self.as_ptr())
        }
    }
}

impl AsNative for SamplerPtr {
    type Native = metal::SamplerStateRef;
    #[inline]
    fn from(native: &metal::SamplerStateRef) -> Self {
        unsafe {
            NonNull::new_unchecked(native.as_ptr())
        }
    }
    #[inline]
    fn as_native(&self) -> &metal::SamplerStateRef {
        unsafe {
            metal::SamplerStateRef::from_ptr(self.as_ptr())
        }
    }
}
