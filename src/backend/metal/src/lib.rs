extern crate gfx_hal as hal;
extern crate metal_rs as metal;
#[macro_use] extern crate bitflags;
extern crate cocoa;
extern crate foreign_types;
#[macro_use] extern crate objc;
extern crate core_graphics;
#[macro_use] extern crate log;
extern crate block;
extern crate dispatch;
extern crate parking_lot;
extern crate smallvec;
extern crate spirv_cross;
extern crate storage_map;

#[cfg(feature = "winit")]
extern crate winit;

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
use std::sync::Arc;
use std::os::raw::c_void;

use hal::queue::QueueFamilyId;

use objc::runtime::{Class, Object};
use core_graphics::geometry::CGRect;
use foreign_types::ForeignTypeRef;
use parking_lot::Mutex;


const MAX_ACTIVE_COMMAND_BUFFERS: usize = 1 << 14;

#[derive(Debug, Clone, Copy)]
pub struct QueueFamily {}

impl hal::QueueFamily for QueueFamily {
    fn queue_type(&self) -> hal::QueueType { hal::QueueType::General }
    fn max_queues(&self) -> usize { 1 }
    fn id(&self) -> QueueFamilyId { QueueFamilyId(0) }
}

struct Shared {
    device: Mutex<metal::Device>,
    queue: Mutex<command::QueueInner>,
    service_pipes: internal::ServicePipes,
    push_constants_buffer_id: u32,
    disabilities: PrivateDisabilities,
}

unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Shared {
    fn new(device: metal::Device) -> Self {
        let feature_macos_10_14: metal::MTLFeatureSet = unsafe { mem::transmute(10004u64) };
        Shared {
            queue: Mutex::new(command::QueueInner::new(&device, Some(MAX_ACTIVE_COMMAND_BUFFERS))),
            service_pipes: internal::ServicePipes::new(&device),
            push_constants_buffer_id: 30,
            disabilities: PrivateDisabilities {
                broken_viewport_near_depth: device.name().starts_with("Intel") &&
                    !device.supports_feature_set(feature_macos_10_14),
            },
            device: Mutex::new(device),
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
                physical_device: device::PhysicalDevice::new(dev),
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
            let view_size: CGRect = msg_send![view, bounds];
            msg_send![render_layer, setFrame: view_size];
            msg_send![view, retain];

            window::SurfaceInner {
                nsview: view,
                render_layer: Mutex::new(render_layer),
            }
        }
    }

    pub fn create_surface_from_nsview(&self, nsview: *mut c_void) -> Surface {
        window::Surface {
            inner: Arc::new(self.create_from_nsview(nsview)),
            apply_pixel_scale: false,
            has_swapchain: false
        }
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        use winit::os::macos::WindowExt;
        window::Surface {
            inner: Arc::new(self.create_from_nsview(window.get_nsview())),
            apply_pixel_scale: true,
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
    type PipelineLayout = native::PipelineLayout;
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type DescriptorPool = native::DescriptorPool;
    type DescriptorSet = native::DescriptorSet;

    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
    type QueryPool = ();
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
    format_depth32_stencil8: bool,
    format_min_srgb_channels: u8,
    format_b5: bool,
    max_buffers_per_stage: usize,
    max_textures_per_stage: usize,
    max_samplers_per_stage: usize,
    buffer_alignment: u64,
    max_buffer_size: u64,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferPtr(*mut metal::MTLBuffer);

impl BufferPtr {
    #[inline]
    pub fn as_native(&self) -> &metal::BufferRef {
        unsafe {
            metal::BufferRef::from_ptr(self.0)
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut metal::MTLBuffer {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturePtr(*mut metal::MTLTexture);

impl TexturePtr {
    #[inline]
    pub fn as_native(&self) -> &metal::TextureRef {
        unsafe {
            metal::TextureRef::from_ptr(self.0)
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut metal::MTLTexture {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerPtr(*mut metal::MTLSamplerState);

impl SamplerPtr {
    #[inline]
    pub fn as_native(&self) -> &metal::SamplerStateRef {
        unsafe {
            metal::SamplerStateRef::from_ptr(self.0)
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut metal::MTLSamplerState {
        self.0
    }
}
