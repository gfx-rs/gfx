// Copyright 2016 The Gfx-rs Developers.
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

#[deny(missing_docs)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate objc;
extern crate cocoa;
extern crate core_foundation;
extern crate core_graphics;
extern crate io_surface;
extern crate winit;
extern crate metal_rs as metal;
extern crate gfx_core as core;
extern crate gfx_device_metal as device_metal;
#[macro_use]
extern crate scopeguard;

use winit::os::macos::WindowExt;

use objc::runtime::{Class, Object, YES};

use cocoa::base::id as cocoa_id;
//use cocoa::base::{selector, class};
use cocoa::foundation::{NSSize};
use cocoa::appkit::{NSWindow, NSView};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::{CFNumber, CFNumberRef};
use core_graphics::base::CGFloat;
use core_graphics::geometry::CGRect;
use io_surface::IOSurface;

use core::{format, handle, memory, texture};
use core::format::{ChannelType, RenderFormat, SurfaceType, Format};
use core::handle::{RawRenderTargetView, RenderTargetView};
use core::memory::Typed;

use device_metal::{native, Resources};

use metal::*;
use std::cell::RefCell;
use std::rc::Rc;

const SWAP_CHAIN_IMAGE_COUNT: usize = 3;

/*
#[derive(Copy, Clone, Debug)]
pub enum InitError {
    /// Unable to create a window.
    Window,
    /// Unable to map format to Metal.
    Format(Format),
    /// The given format is present in Metal, but not allowed by the backbuffer.
    BackbufferFormat(Format),
    /// Unable to find a supported driver type.
    DriverType,
}

/// Initialize with a given size. Raw format version.
pub fn init_raw(wb: winit::WindowBuilder, events_loop: &winit::EventsLoop, color_format: Format)
        -> Result<(MetalWindow, Device, Factory, RawRenderTargetView<Resources>), InitError>
{
    use device_metal::map_format;

    let winit_window = wb.build(events_loop).unwrap();

    unsafe {
        let wnd: cocoa_id = mem::transmute(winit_window.get_nswindow());

        let layer = CAMetalLayer::new();
        let desired_pixel_format = match map_format(color_format, true) {
            Some(fm) => fm,
            None => return Err(InitError::Format(color_format)),
        };
        match desired_pixel_format {
            MTLPixelFormat::BGRA8Unorm | MTLPixelFormat::BGRA8Unorm_sRGB | MTLPixelFormat::RGBA16Float => {
                layer.set_pixel_format(desired_pixel_format);
            },
            _ => return Err(InitError::BackbufferFormat(color_format)),
        }
        let draw_size = winit_window.get_inner_size().unwrap();
        layer.set_edge_antialiasing_mask(0);
        layer.set_masks_to_bounds(true);
        //layer.set_magnification_filter(kCAFilterNearest);
        //layer.set_minification_filter(kCAFilterNearest);
        layer.set_drawable_size(NSSize::new(draw_size.0 as f64, draw_size.1 as f64));
        layer.set_presents_with_transaction(false);
        layer.remove_all_animations();

        let view = wnd.contentView();
        view.setWantsLayer(YES);
        view.setLayer(mem::transmute(layer.0));

        let (device, factory, color, daddr, addr) = device_metal::create(color_format, draw_size.0, draw_size.1).unwrap();
        layer.set_device(device.device);

        let drawable = layer.next_drawable().unwrap();

        let window = MetalWindow {
            window: winit_window,
            layer: layer,
            drawable: daddr,
            backbuffer: addr,
            pool: Cell::new(NSAutoreleasePool::alloc().init())
        };

        (*daddr).0 = drawable.0;
        (*addr).0 = drawable.texture().0;

        Ok((window, device, factory, color))
    }
}
*/

fn get_format_bytes_per_pixel(format: MTLPixelFormat) -> usize {
    // TODO: more formats
    match format {
        MTLPixelFormat::RGBA8Unorm => 4,
        MTLPixelFormat::RGBA8Unorm_sRGB => 4,
        MTLPixelFormat::BGRA8Unorm => 4,
        _ => unimplemented!(),
    }
}

pub struct Surface {
    raw: Rc<SurfaceInner>,
    manager: handle::Manager<Resources>,
}

struct SurfaceInner {
    nsview: *mut Object,
    render_layer: *mut Object,
}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe { msg_send![self.nsview, release]; }
    }
}

impl core::Surface<device_metal::Backend> for Surface {
    type Swapchain = Swapchain;

    fn supports_queue(&self, queue_family: &device_metal::QueueFamily) -> bool {
        true
    }

    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, present_queue: &Q) -> Self::Swapchain
        where Q: AsRef<device_metal::CommandQueue>
    {
        let (mtl_format, cv_format) = match config.color_format {
            format::Format(SurfaceType::R8_G8_B8_A8, ChannelType::Srgb) => (MTLPixelFormat::RGBA8Unorm_sRGB, native::kCVPixelFormatType_32RGBA),
            _ => panic!("unsupported backbuffer format"), // TODO: more formats
        };

        let render_layer = self.raw.render_layer;
        let nsview = self.raw.nsview;
        let queue = present_queue.as_ref();

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
            let pixel_size = get_format_bytes_per_pixel(mtl_format) as u64;

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

            let backbuffers = io_surfaces.iter().map(|surface| {
                use core::handle::Producer;
                let mapped_texture: MTLTexture = msg_send![device.0, newTextureWithDescriptor: backbuffer_descriptor.0 iosurface: surface.obj plane: 0];
                let color = self.manager.make_texture(
                    device_metal::native::Texture(
                        device_metal::native::RawTexture(Box::into_raw(Box::new(mapped_texture))),
                        memory::Usage::Data,
                    ),
                    texture::Info {
                        levels: 1,
                        kind: texture::Kind::D2(pixel_width as u16, pixel_height as u16, texture::AaMode::Single),
                        format: config.color_format.0,
                        bind: memory::RENDER_TARGET | memory::TRANSFER_SRC,
                        usage: memory::Usage::Data,
                    },
                );

                // TODO: depth-stencil

                (color, None)
            }).collect();

            Swapchain {
                surface: self.raw.clone(),
                pixel_width,
                pixel_height,

                io_surfaces,
                backbuffers,
                frame_index: 0,
                present_index: 0,
            }
        }
    }
}

pub struct Swapchain {
    surface: Rc<SurfaceInner>,
    pixel_width: u64,
    pixel_height: u64,

    io_surfaces: Vec<IOSurface>,
    backbuffers: Vec<core::Backbuffer<device_metal::Backend>>,
    frame_index: usize,
    present_index: usize,
}

impl core::Swapchain<device_metal::Backend> for Swapchain {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<device_metal::Backend>] {
        &self.backbuffers
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<device_metal::Resources>) -> core::Frame {
        unsafe {
            // TODO: sync
            /*
            match sync {
                core::FrameSync::Semaphore(semaphore) => {
                    // FIXME: this is definitely wrong
                    native::dispatch_semaphore_signal(semaphore.0);
                },
                core::FrameSync::Fence(_fence) => {
                    // TODO: unimplemented!(),
                    warn!("Fence based frame acquisition not implemented");
                }
            }
            */

            let frame = core::Frame::new(self.frame_index % self.backbuffers.len());
            self.frame_index += 1;
            frame
        }
    }

    fn present<Q>(&mut self, present_queue: &mut Q, wait_semaphores: &[&handle::Semaphore<device_metal::Resources>])
        where Q: AsMut<device_metal::CommandQueue>
    {
        let buffer_index = self.present_index % self.io_surfaces.len();

        unsafe {
            let io_surface = &mut self.io_surfaces[buffer_index];
            msg_send![self.surface.render_layer, setContents: io_surface.obj];
        }

        self.present_index += 1;
    }
}

pub struct Window(pub winit::Window);

impl core::WindowExt<device_metal::Backend> for Window {
    type Surface = Surface;
    type Adapter = device_metal::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface, Vec<device_metal::Adapter>) {
        let surface = create_surface(&self.0);
        let adapters = device_metal::enumerate_adapters();
        (surface, adapters)
    }
}

fn create_surface(window: &winit::Window) -> Surface {
    unsafe {
        let wnd: cocoa::base::id = std::mem::transmute(window.get_nswindow());

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
        Surface {
            raw: Rc::new(
                    SurfaceInner {
                        nsview: view,
                        render_layer,
                    }),
            manager: handle::Manager::new(),
        }
    }
}
