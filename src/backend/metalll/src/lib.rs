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

extern crate gfx_corell as core;
extern crate winit;
extern crate cocoa;
#[macro_use] extern crate objc;
extern crate io_surface;
extern crate core_foundation;
extern crate core_graphics;
extern crate cgl;
extern crate gfx_gl as gl;
#[macro_use] extern crate log;
#[macro_use] extern crate scopeguard;
extern crate block;

extern crate metal_rs as metal;

mod command;
mod factory;
mod native;
mod conversions;

pub use command::{QueueFamily, CommandQueue, CommandPool, RenderPassInlineEncoder};
pub use factory::{Factory};

pub type GraphicsCommandPool = CommandPool;

use std::str::FromStr;
use std::mem;
use std::marker::PhantomData;
use std::cell::RefCell;
use std::rc::{Rc, Weak as WeakRc};
use std::sync::Arc;

use core::{format, memory};
use core::format::SurfaceType;
use core::format::ChannelType;
use metal::*;
use winit::os::macos::WindowExt;
use objc::runtime::Class;
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::{CFNumber, CFNumberRef};
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use cocoa::base::{nil, YES};
use cocoa::appkit::{self, NSWindow, NSView, NSOpenGLPixelFormat, NSOpenGLContext};
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

pub struct Surface {
    nsview: cocoa::base::id,
    swap_chain: RefCell<WeakRc<SwapChainInner>>,
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { msg_send![self.nsview, release]; }
    }
}

pub struct SwapChain(Rc<SwapChainInner>);

struct SwapChainInner {
    ns_glcontext: cocoa::base::id,
    gl: Box<gl::Gl>,
    pixel_width: u64,
    pixel_height: u64,

    io_surfaces: Vec<IOSurface>,
    images: Vec<native::Image>,
    frame_index: RefCell<usize>,
}

impl Drop for SwapChainInner {
    fn drop(&mut self) {
        unsafe { msg_send![self.ns_glcontext, release] }
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

            msg_send![view, retain];
            Surface {
                nsview: view,
                swap_chain: Default::default(),
            }
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

        let heap_types;
        let memory_heaps;

        #[cfg(not(feature = "native_heap"))]
        {
            // TODO: heap types for each memory binding
            heap_types = vec![core::HeapType {
                id: 0,
                properties: memory::HeapProperties::all(),
                heap_index: 0,
            }];
            memory_heaps = Vec::new();
        }

        core::Device {
            factory,
            general_queues,
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            heap_types,
            memory_heaps,
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
    type SwapChain = SwapChain;

    fn build_swapchain<T: format::RenderFormat>(&self, queue: &CommandQueue) -> SwapChain {
        if let Some(_) = self.swap_chain.borrow().upgrade() {
            panic!("multiple swap chains with the same surface are not supported")
        }

        let (mtl_format, cv_format) = match T::get_format() {
            format::Format(SurfaceType::R8_G8_B8_A8, ChannelType::Srgb) => (MTLPixelFormat::RGBA8Unorm_sRGB, kCVPixelFormatType_32RGBA),
            _ => panic!("unsupported backbuffer format"), // TODO: more formats
        };

        let inner = unsafe {
            let view_points_size: CGRect = msg_send![self.nsview, bounds];
            let view_window: cocoa::base::id = msg_send![self.nsview, window];
            let scale_factor: CGFloat = msg_send![view_window, backingScaleFactor];
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

            // Create OpenGL context for compositing
            // TODO: CAMetalLayer is able to composite IOSurfaces directly, so we should try
            // to figure out how to do the same.
            let gl_pixelformat = NSOpenGLPixelFormat::alloc(nil).initWithAttributes_(&[
                appkit::NSOpenGLPFAOpenGLProfile as u32, appkit::NSOpenGLProfileVersion3_2Core as u32,
                appkit::NSOpenGLPFAColorSize as u32, 24, // FIXME
                appkit::NSOpenGLPFAAlphaSize as u32, 8,
                // TODO: float buffers
                0,
            ]); // Returns retained
            defer! { msg_send![gl_pixelformat, release] };
            let ns_glcontext = NSOpenGLContext::alloc(nil).initWithFormat_shareContext_(gl_pixelformat, nil);
            defer_on_unwind! { msg_send![ns_glcontext, release] }
            if ns_glcontext.is_null() {
                panic!("failed to create NSOpenGLContext");
            }
            ns_glcontext.setView_(self.nsview);
            let framework_name: CFString = FromStr::from_str("com.apple.opengl").unwrap();
            let framework = CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef());
            let gl = Box::new(gl::Gl::load_with(|name| {
                let symbol_name: CFString = FromStr::from_str(name).unwrap();
                CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef()) as *const _
            }));

            Rc::new(SwapChainInner {
                ns_glcontext,
                gl,
                pixel_width,
                pixel_height,

                io_surfaces,
                images,
                frame_index: RefCell::new(0),
            })
        };

        *self.swap_chain.borrow_mut() = Rc::downgrade(&inner);

        SwapChain(inner)
    }
}

impl core::SwapChain for SwapChain {
    type R = Resources;
    type Image = native::Image;

    fn get_images(&mut self) -> &[native::Image] {
        &self.0.images
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

            let mut frame_index = self.0.frame_index.borrow_mut();
            let frame = core::Frame::new(*frame_index % self.0.images.len());
            *frame_index += 1;
            frame
        }
    }

    fn present(&mut self) {
        let frame_index = *self.0.frame_index.borrow();
        if frame_index == 0 {
            panic!("no frame to present");
        }
        let buffer_index = (frame_index - 1) % self.0.io_surfaces.len();

        // Draw IOSurface to OpenGL context
        unsafe {
            let io_surface = &mut *(&self.0.io_surfaces[buffer_index] as *const IOSurface as *mut IOSurface);

            let cgl_context: cgl::CGLContextObj = msg_send![self.0.ns_glcontext, CGLContextObj];
            cgl::CGLSetCurrentContext(cgl_context);

            self.0.gl.ClearColor(1.0, 1.0, 1.0, 1.0);
            self.0.gl.Clear(gl::COLOR_BUFFER_BIT);

            let mut gl_surface_tex: gl::types::GLuint = 0;
            self.0.gl.GenTextures(1, &mut gl_surface_tex);

            self.0.gl.BindTexture(gl::TEXTURE_RECTANGLE, gl_surface_tex);
            io_surface.bind_to_gl_texture(self.0.pixel_width as i32, self.0.pixel_height as i32);
            self.0.gl.TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            self.0.gl.TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);

            // Bind IOSurface texture as framebuffer
            self.0.gl.BindTexture(gl::TEXTURE_RECTANGLE, 0);
            let mut gl_surface_fb: gl::types::GLuint = 0;
            self.0.gl.GenFramebuffers(1, &mut gl_surface_fb);
            self.0.gl.BindFramebuffer(gl::FRAMEBUFFER, gl_surface_fb);
            self.0.gl.FramebufferTexture2D(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_RECTANGLE, gl_surface_tex, 0);
            self.0.gl.BindFramebuffer(gl::FRAMEBUFFER, 0);

            // Blit!
            self.0.gl.BindFramebuffer(gl::READ_FRAMEBUFFER, gl_surface_fb);
            self.0.gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, 0);
            self.0.gl.BlitFramebuffer(
                0, 0, self.0.pixel_width as i32, self.0.pixel_height as i32,
                0, 0, self.0.pixel_width as i32, self.0.pixel_height as i32,
                gl::COLOR_BUFFER_BIT,
                gl::NEAREST
            );
            self.0.gl.BindFramebuffer(gl::FRAMEBUFFER, 0);

            self.0.gl.Flush();

            self.0.gl.DeleteFramebuffers(1, &gl_surface_fb);
            self.0.gl.DeleteTextures(1, &gl_surface_tex);
        }
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

const kCVPixelFormatType_32RGBA: u32 = (b'R' as u32) << 24 | (b'G' as u32) << 16 | (b'B' as u32) << 8 | b'C' as u32;
