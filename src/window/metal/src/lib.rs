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
extern crate objc;
extern crate cocoa;
extern crate winit;
extern crate metal;
extern crate gfx_core;
extern crate gfx_device_metal;

use winit::os::macos::WindowExt;

use objc::runtime::{Object, Class, BOOL, YES, NO};

use cocoa::base::id as cocoa_id;
use cocoa::base::{selector, class};
use cocoa::foundation::{NSUInteger};
use cocoa::appkit::{NSApp,
                    NSApplication, NSApplicationActivationPolicyRegular,
                    NSWindow, NSTitledWindowMask, NSBackingStoreBuffered,
                    NSMenu, NSMenuItem, NSRunningApplication, NSView,
                    NSApplicationActivateIgnoringOtherApps};

use gfx_core::tex::Size;
use gfx_core::format::Format;
use gfx_core::handle::RawRenderTargetView;

use gfx_device_metal::{Device, Factory, Resources};

use metal::{CAMetalLayer};

use std::mem;

pub struct Window {
    handle: cocoa_id,
    layer: CAMetalLayer,
}

#[derive(Copy, Clone, Debug)]
pub enum InitError {
    /// Unable to create a window.
    Window,
    /// Unable to map format to Metal.
    Format(Format),
    /// Unable to find a supported driver type.
    DriverType,
}

/// Initialize with a given size. Raw format version.
pub fn init_raw(title: &str, requested_width: u32, requested_height: u32, color_format: Format)
        -> Result<(Window, Device, Factory, RawRenderTargetView<Resources>), InitError> {
    let winit_wnd = winit::WindowBuilder::new()
        .with_dimensions(requested_width, requested_height)
        .with_title(title.to_string()).build().unwrap();

    unsafe {
        let wnd: cocoa_id = mem::transmute(winit_wnd.get_nswindow());

        let layer = CAMetalLayer::layer();
        layer.set_pixel_format(match gfx_device_metal::map_format(color_format, true) {
            Some(fm) => fm,
            None => return Err(InitError::Format(color_format)),
        });

        let view = wnd.contentView();
        view.setWantsLayer(YES);
        // view.setLayer(...);

        
    }
    
    Err(InitError::Window)
}
