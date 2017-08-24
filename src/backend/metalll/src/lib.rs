// Copyright 2017 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[macro_use] extern crate gfx_corell as core;
extern crate winit;
extern crate cocoa;
#[macro_use] extern crate objc;
extern crate io_surface;
extern crate core_foundation;
extern crate core_graphics;
#[macro_use] extern crate log;
#[macro_use] extern crate scopeguard;
extern crate block;

extern crate metal_rs as metal;

mod command;
mod factory;
mod native;
mod conversions;

pub use command::{QueueFamily, CommandQueue, CommandPool};
pub use factory::{Factory, LanguageVersion};

pub type GraphicsCommandPool = CommandPool;

use std::mem;
use std::marker::PhantomData;
use std::cell::RefCell;
use std::rc::Rc;

use core::{format, memory};
use core::format::SurfaceType;
use core::format::ChannelType;
use metal::*;
use winit::os::macos::WindowExt;
use objc::runtime::{Object, Class};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::{CFNumber, CFNumberRef};
use cocoa::base::YES;
use cocoa::appkit::NSWindow;
use core_graphics::base::CGFloat;
use core_graphics::geometry::CGRect;
use io_surface::IOSurface;

pub struct Instance {
}

pub struct Adapter {
    device: MTLDevice,
    adapter_info: core::AdapterInfo,
    queue_families: [QueueFamily; 1],
}

impl Drop for Adapter {
    fn drop(&mut self) {
        unsafe { self.device.release(); }
    }
}

pub struct Surface(Rc<SurfaceInner>);

struct SurfaceInner {
    nsview: *mut Object,
    render_layer: RefCell<*mut Object>,
}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe { msg_send![self.nsview, release]; }
    }
}

pub struct Swapchain {
    surface: Rc<SurfaceInner>,
    pixel_width: u64,
    pixel_height: u64,

    io_surfaces: Vec<IOSurface>,
    images: Vec<native::Image>,
    frame_index: usize,
    present_index: usize,
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            for image in self.images.drain(..) {
                image.0.release();
            }
        }
    }
}

const SWAP_CHAIN_IMAGE_COUNT: usize = 3;

#[derive(Debug, Clone, Hash)]
pub enum Resources {}

impl core::Instance for Instance {
    type Adapter = Adapter;
    type Surface = Surface;
    type Window = winit::Window;

    fn create() -> Self {
        Instance {}
    }

    fn enumerate_adapters(&self) -> Vec<Self::Adapter> {
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
            queue_families: [QueueFamily{}],
        }]
    }

    fn create_surface(&self, window: &winit::Window) -> Self::Surface {
        unsafe {
            let wnd: cocoa::base::id = mem::transmute(window.get_nswindow());

            let view = wnd.contentView();
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
            Surface(Rc::new(SurfaceInner {
                nsview: view,
                render_layer: RefCell::new(render_layer),
            }))
        }
    }
}

impl core::Adapter for Adapter {
    type CommandQueue = CommandQueue;
    type QueueFamily = QueueFamily;
    type Factory = Factory;
    type Resources = Resources;

    fn open<'a, I>(&self, mut queue_descs: I) -> core::Device<Self::Resources, Self::Factory, Self::CommandQueue>
        where I: ExactSizeIterator<Item=(&'a Self::QueueFamily, u32)>
    {
        if queue_descs.len() != 1 {
            panic!("Metal only supports one queue family");
        }
        let (_, queue_count) = queue_descs.next().unwrap();

        let factory = factory::create_factory(self.device);
        let general_queues = (0..queue_count).map(|_| {
            unsafe { core::GeneralQueue::new(command::CommandQueue::new(self.device)) }
        }).collect();

        let heap_types = vec![
            core::HeapType {
                id: 0,
                properties: memory::CPU_VISIBLE | memory::CPU_CACHED,
                heap_index: 0,
            },
            core::HeapType {
                id: 1,
                properties: memory::CPU_VISIBLE | memory::CPU_CACHED | memory::WRITE_COMBINED,
                heap_index: 1,
            },
            core::HeapType {
                id: 2,
                properties: memory::CPU_VISIBLE | memory::COHERENT | memory::CPU_CACHED,
                heap_index: 2,
            },
            core::HeapType {
                id: 3,
                properties: memory::CPU_VISIBLE | memory::COHERENT 
                    | memory::CPU_CACHED | memory::WRITE_COMBINED,
                heap_index: 3,
            },
            core::HeapType {
                id: 4,
                properties: memory::DEVICE_LOCAL,
                heap_index: 4,
            },
        ];
        let memory_heaps = Vec::new();

        core::Device {
            factory,
            general_queues,
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            heap_types,
            memory_heaps,
            caps: core::Capabilities {
                heterogeneous_resource_heaps: true,
                buffer_copy_offset_alignment: 1,
                buffer_copy_row_pitch_alignment: 1,
            },
            _marker: PhantomData,
        }
    }

    fn get_info(&self) -> &core::AdapterInfo {
        &self.adapter_info
    }

    fn get_queue_families(&self) -> std::slice::Iter<Self::QueueFamily> {
        self.queue_families.iter()
    }
}

impl core::Surface for Surface {
    type Queue = CommandQueue;
    type Swapchain = Swapchain;

    fn build_swapchain<T: format::RenderFormat>(&self, queue: &CommandQueue) -> Swapchain {
        let (mtl_format, cv_format) = match T::get_format() {
            format::Format(SurfaceType::R8_G8_B8_A8, ChannelType::Srgb) => (MTLPixelFormat::RGBA8Unorm_sRGB, native::kCVPixelFormatType_32RGBA),
            _ => panic!("unsupported backbuffer format"), // TODO: more formats
        };

        let render_layer_borrow = self.0.render_layer.borrow_mut();
        let render_layer = *render_layer_borrow;
        let nsview = self.0.nsview;

        unsafe {
            // Update render layer size
            let view_points_size: CGRect = msg_send![nsview, bounds];
            msg_send![render_layer, setBounds: view_points_size];
            let view_window: *mut Object = msg_send![nsview, window];
            if view_window.is_null() {
                panic!("surface is not attached to a window");
            }
            let scale_factor: CGFloat = msg_send![view_window, backingScaleFactor];
            msg_send![render_layer, setContentsScale: scale_factor];
            let pixel_width = (view_points_size.size.width * scale_factor) as u64;
            let pixel_height = (view_points_size.size.height * scale_factor) as u64;
            let pixel_size = conversions::get_format_bytes_per_pixel(mtl_format) as u64;

            info!("allocating {} IOSurface backbuffers of size {}x{} with pixel format 0x{:x}", SWAP_CHAIN_IMAGE_COUNT, pixel_width, pixel_height, cv_format);
            // Create swap chain surfaces
            let io_surfaces: Vec<_> = (0..SWAP_CHAIN_IMAGE_COUNT).map(|_| {
                io_surface::new(&CFDictionary::from_CFType_pairs::<CFStringRef, CFNumberRef, CFString, CFNumber>(&[
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceWidth), CFNumber::from_i32(pixel_width as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceHeight), CFNumber::from_i32(pixel_height as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceBytesPerRow), CFNumber::from_i32((pixel_width * pixel_size) as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceBytesPerElement), CFNumber::from_i32(pixel_size as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfacePixelFormat), CFNumber::from_i32(cv_format as i32)),
                ]))
            }).collect();

            let device = queue.device();

            let backbuffer_descriptor = MTLTextureDescriptor::new();
            defer! { backbuffer_descriptor.release() };
            backbuffer_descriptor.set_pixel_format(mtl_format);
            backbuffer_descriptor.set_width(pixel_width as u64);
            backbuffer_descriptor.set_height(pixel_height as u64);
            backbuffer_descriptor.set_usage(MTLTextureUsageRenderTarget);

            let images = io_surfaces.iter().map(|surface| {
                let mapped_texture: MTLTexture = msg_send![device.0, newTextureWithDescriptor: backbuffer_descriptor.0 iosurface: surface.obj plane: 0];
                native::Image(mapped_texture)
            }).collect();

            Swapchain {
                surface: self.0.clone(),
                pixel_width,
                pixel_height,

                io_surfaces,
                images,
                frame_index: 0,
                present_index: 0,
            }
        }
    }
}

impl core::Swapchain for Swapchain {
    type R = Resources;
    type Image = native::Image;

    fn get_images(&mut self) -> &[native::Image] {
        &self.images
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<Resources>) -> core::Frame {
        unsafe {
            match sync {
                core::FrameSync::Semaphore(semaphore) => {
                    // FIXME: this is definitely wrong
                    native::dispatch_semaphore_signal(semaphore.0);
                },
                core::FrameSync::Fence(_fence) => unimplemented!(),
            }

            let frame = core::Frame::new(self.frame_index % self.images.len());
            self.frame_index += 1;
            frame
        }
    }

    fn present(&mut self) {
        let buffer_index = self.present_index % self.io_surfaces.len();

        unsafe {
            let io_surface = &mut self.io_surfaces[buffer_index];
            let render_layer_borrow = self.surface.render_layer.borrow_mut();
            let render_layer = *render_layer_borrow;
            msg_send![render_layer, setContents: io_surface.obj];
        }

        self.present_index += 1;
    }
}

impl core::Resources for Resources {
    type ShaderLib = native::ShaderLib;
    type RenderPass = native::RenderPass;
    type PipelineLayout = native::PipelineLayout;
    type FrameBuffer = native::FrameBuffer;
    type GraphicsPipeline = native::GraphicsPipeline;
    type ComputePipeline = native::ComputePipeline;
    type UnboundBuffer = native::UnboundBuffer;
    type Buffer = native::Buffer;
    type UnboundImage = native::UnboundImage;
    type Image = native::Image;
    type ConstantBufferView = native::ConstantBufferView;
    type ShaderResourceView = native::ShaderResourceView;
    type UnorderedAccessView = native::UnorderedAccessView;
    type RenderTargetView = native::RenderTargetView;
    type DepthStencilView = native::DepthStencilView;
    type Sampler = native::Sampler;
    type Semaphore = native::Semaphore;
    type Fence = native::Fence;
    type Heap = native::Heap;
    type Mapping = native::Mapping;
    type DescriptorHeap = native::DescriptorHeap;
    type DescriptorSetPool = native::DescriptorSetPool;
    type DescriptorSet = native::DescriptorSet;
    type DescriptorSetLayout = native::DescriptorSetLayout;

}

