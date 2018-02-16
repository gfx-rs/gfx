extern crate gfx_hal as hal;
extern crate cocoa;
extern crate foreign_types;
#[macro_use] extern crate objc;
extern crate io_surface;
extern crate core_foundation;
extern crate core_graphics;
#[macro_use] extern crate log;
extern crate block;
extern crate spirv_cross;

extern crate metal_rs as metal;

#[cfg(feature = "winit")]
extern crate winit;

mod device;
mod window;
mod command;
mod native;
mod conversions;

pub use command::{CommandQueue, CommandPool};
pub use device::LanguageVersion;
pub use window::{Surface, Swapchain};

pub type GraphicsCommandPool = CommandPool;

use std::mem;
use std::cell::RefCell;
use std::rc::Rc;
use std::os::raw::c_void;

use hal::queue::QueueFamilyId;

use objc::runtime::{Object, Class};
use cocoa::base::YES;
use cocoa::foundation::NSAutoreleasePool;
use core_graphics::geometry::CGRect;


#[derive(Debug, Clone, Copy)]
pub struct QueueFamily {}

impl hal::QueueFamily for QueueFamily {
    fn queue_type(&self) -> hal::QueueType { hal::QueueType::General }
    fn max_queues(&self) -> usize { 1 }
    fn id(&self) -> QueueFamilyId { QueueFamilyId(0) }
}

pub struct Instance {}

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        // TODO: enumerate all devices

        let device = metal::Device::system_default();

        vec![
            hal::Adapter {
                info: hal::AdapterInfo {
                    name: device.name().into(),
                    vendor: 0,
                    device: 0,
                    software_rendering: false,
                },
                physical_device: device::PhysicalDevice::new(device),
                queue_families: vec![QueueFamily{}],
            }
        ]
    }
}

impl Instance {
    pub fn create(_: &str, _: u32) -> Self {
        Instance {}
    }

    pub fn create_surface_from_nsview(&self, nsview: *mut c_void) -> Surface {
        unsafe {
            let view: cocoa::base::id = mem::transmute(nsview);
            if view.is_null() {
                panic!("window does not have a valid contentView");
            }

            msg_send![view, setWantsLayer: YES];
            let render_layer: *mut Object = msg_send![Class::get("CALayer").unwrap(), new]; // Returns retained
            let view_size: CGRect = msg_send![view, bounds];
            msg_send![render_layer, setFrame: view_size];
            let view_layer: *mut Object = msg_send![view, layer];
            msg_send![view_layer, addSublayer: render_layer];

            msg_send![view, retain];
            window::Surface(Rc::new(window::SurfaceInner {
                nsview: view,
                render_layer: RefCell::new(render_layer),
            }))
        }
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        use winit::os::macos::WindowExt;
        self.create_surface_from_nsview(window.get_nsview())
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
    type CommandQueue = CommandQueue;
    type CommandBuffer = command::CommandBuffer;

    type Memory = native::Memory;
    type CommandPool = command::CommandPool;

    type ShaderModule = native::ShaderModule;
    type RenderPass = native::RenderPass;
    type Framebuffer = native::FrameBuffer;

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

pub struct AutoreleasePool {
    pool: cocoa::base::id,
}

impl Drop for AutoreleasePool {
    fn drop(&mut self) {
        unsafe { self.pool.drain() }
    }
}

impl AutoreleasePool {
    pub unsafe fn new() -> Self {
        AutoreleasePool {
            pool: NSAutoreleasePool::new(cocoa::base::nil),
        }
    }

    pub unsafe fn reset(&mut self) {
        self.pool.drain();
        self.pool = NSAutoreleasePool::new(cocoa::base::nil);
    }
}
