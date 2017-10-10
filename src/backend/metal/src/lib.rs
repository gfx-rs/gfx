extern crate gfx_core as core;
extern crate cocoa;
#[macro_use] extern crate objc;
extern crate io_surface;
extern crate core_foundation;
extern crate core_graphics;
#[macro_use] extern crate log;
#[macro_use] extern crate scopeguard;
extern crate block;

extern crate metal_rs as metal;

#[cfg(feature = "winit")]
extern crate winit;

mod device;
mod window;
mod command;
mod native;
mod conversions;

pub use command::{CommandQueue, CommandPool};
pub use device::{Adapter, LanguageVersion};
pub use window::{Surface, Swapchain};

pub type GraphicsCommandPool = CommandPool;

use std::mem;
use std::cell::RefCell;
use std::rc::Rc;
use std::os::raw::c_void;

use core::{QueueType};
use objc::runtime::{Object, Class};
use cocoa::base::YES;
use core_graphics::geometry::CGRect;

pub struct Instance {
}

impl core::Instance<Backend> for Instance {
    fn enumerate_adapters(&self) -> Vec<Adapter> {
        // TODO: enumerate all devices

        let device = metal::create_system_default_device(); // Returns retained

        vec![Adapter {
            device,
            adapter_info: core::AdapterInfo {
                name: device.name().into(),
                vendor: 0,
                device: 0,
                software_rendering: false,
            },
            queue_families: [(native::QueueFamily{}, QueueType::General)],
        }]
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
impl core::Backend for Backend {
    type Adapter = device::Adapter;
    type Device = device::Device;

    type Surface = window::Surface;
    type Swapchain = window::Swapchain;

    type CommandQueue = CommandQueue;
    type CommandBuffer = command::CommandBuffer;
    type SubpassCommandBuffer = command::CommandBuffer;
    type QueueFamily = native::QueueFamily;

    type Memory = native::Memory;
    type CommandPool = command::CommandPool;
    type SubpassCommandPool = command::CommandPool;

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
}

