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

//#[macro_use]
//extern crate log;
//#[macro_use]
extern crate objc;
extern crate cocoa;
extern crate winit;
extern crate metal_rs as metal;
extern crate gfx_core as core;
extern crate gfx_device_metal as device_metal;

use winit::os::macos::WindowExt;

use objc::runtime::{YES};

use cocoa::base::id as cocoa_id;
//use cocoa::base::{selector, class};
use cocoa::foundation::{NSSize};
use cocoa::appkit::{NSWindow, NSView};

use core::format::{RenderFormat, Format};
use core::handle::{RawRenderTargetView, RenderTargetView};
use core::memory::Typed;

use device_metal::{Device, Factory, Resources};

use metal::*;

//use winit::{Window};

use std::ops::Deref;
use std::cell::Cell;
use std::error::Error;
use std::fmt;
use std::mem;

pub struct MetalWindow {
    window: winit::Window,
    layer: CAMetalLayer,
    drawable: *mut CAMetalDrawable,
    backbuffer: *mut MTLTexture,
    pool: Cell<NSAutoreleasePool>
}

impl Deref for MetalWindow {
    type Target = winit::Window;

    fn deref(&self) -> &winit::Window {
        &self.window
    }
}

impl MetalWindow {
    pub fn swap_buffers(&self) -> Result<(), ()> {
        // TODO: did we fail to swap buffers?
        // TODO: come up with alternative to this hack

        unsafe {
            self.pool.get().release();
            self.pool.set(NSAutoreleasePool::alloc().init());

            let drawable = self.layer.next_drawable().unwrap();
            //drawable.retain();

            *self.drawable = drawable;

            *self.backbuffer = drawable.texture();
        }

        Ok(())
    }
}


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

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            InitError::Format(ref fm) => write!(f, "{}: {:?}", self.description(), fm),
            InitError::BackbufferFormat(ref fm) => write!(f, "{}: {:?}", self.description(), fm),
            _ => f.write_str(self.description()),
        }
    }
}

impl Error for InitError {
    fn description(&self) -> &str {
        match *self {
            InitError::Window => "unable to create a window",
            InitError::Format(_) => "unable to map format",
            InitError::BackbufferFormat(_) => "format not allowed by the backbuffer",
            InitError::DriverType => "unable to find a supported driver type",
        }
    }
}

pub fn init<C: RenderFormat>(wb: winit::WindowBuilder, events_loop: &winit::EventsLoop)
        -> Result<(MetalWindow, Device, Factory, RenderTargetView<Resources, C>), InitError>
{
    init_raw(wb, events_loop, C::get_format())
        .map(|(window, device, factory, color)| (window, device, factory, Typed::new(color)))
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
        let logical_size = winit_window.get_inner_size().unwrap();
        let physical_size = logical_size.to_physical(winit_window.get_hidpi_factor());
        layer.set_edge_antialiasing_mask(0);
        layer.set_masks_to_bounds(true);
        //layer.set_magnification_filter(kCAFilterNearest);
        //layer.set_minification_filter(kCAFilterNearest);
        layer.set_drawable_size(NSSize::new(physical_size.width, physical_size.height));
        layer.set_presents_with_transaction(false);
        layer.remove_all_animations();

        let view = wnd.contentView();
        view.setWantsLayer(YES);
        view.setLayer(mem::transmute(layer.0));

        let (draw_width, draw_height): (u32, u32) = physical_size.into();
        let (device, factory, color, daddr, addr) = device_metal::create(color_format, draw_width, draw_height).unwrap();
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
