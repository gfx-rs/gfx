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

extern crate winit;
extern crate xcb;
extern crate vk_sys as vk;
extern crate gfx_core;
extern crate gfx_device_vulkan;

use std::{mem, ptr};
use gfx_core::format;


pub fn init_winit(builder: winit::WindowBuilder) -> (winit::Window, gfx_device_vulkan::GraphicsQueue, gfx_device_vulkan::Factory) {
    let (device, factory, _backend) = gfx_device_vulkan::create(&builder.window.title, 1, &[],
        &["VK_KHR_surface", "VK_KHR_xcb_surface"], &["VK_KHR_swapchain"]);
    let win = builder.build().unwrap();
    (win, device, factory)
}

pub type TargetHandle = gfx_core::handle::RenderTargetView<gfx_device_vulkan::Resources, format::Rgba8>;

pub struct SwapTarget {
    _image: vk::Image,
    target: TargetHandle,
    _fence: vk::Fence,
}

pub struct Window {
    connection: xcb::Connection,
    _foreground: u32,
    window: u32,
    swapchain: vk::SwapchainKHR,
    targets: Vec<SwapTarget>,
    queue: gfx_device_vulkan::GraphicsQueue,
}

pub struct Frame<'a> {
    window: &'a mut Window,
    target_id: u32,
}

impl<'a> Frame<'a> {
    pub fn get_target(&self) -> TargetHandle {
        self.window.targets[self.target_id as usize].target.clone()
    }
    pub fn get_queue(&mut self) -> &mut gfx_device_vulkan::GraphicsQueue {
        &mut self.window.queue
    }
}

impl<'a> Drop for Frame<'a> {
    fn drop(&mut self) {
        let mut result = vk::SUCCESS;
        let info = vk::PresentInfoKHR {
            sType: vk::STRUCTURE_TYPE_PRESENT_INFO_KHR,
            pNext: ptr::null(),
            waitSemaphoreCount: 0,
            pWaitSemaphores: ptr::null(),
            swapchainCount: 1,
            pSwapchains: &self.window.swapchain,
            pImageIndices: &self.target_id,
            pResults: &mut result,
        };
        let (_dev, vk) = self.window.queue.get_share().get_device();
        unsafe {
            vk.QueuePresentKHR(self.window.queue.get_queue(), &info);
        }
        assert_eq!(vk::SUCCESS, result);
    }
}

impl Window {
    pub fn wait_draw(&mut self) -> Result<Option<Frame>, ()> {
        let ev = match self.connection.wait_for_event() {
            Some(ev) => ev,
            None => return Err(()),
        };
        //self.connection.flush();
        match ev.response_type() & 0x80 {
            xcb::EXPOSE => Ok(Some(self.start_frame())),
            xcb::KEY_PRESS => Err(()),
            _ => Ok(None)
        }
    }

    pub fn start_frame(&mut self) -> Frame {
        //TODO: handle window resize
        let index = unsafe {
            let (dev, vk) = self.queue.get_share().get_device();
            let mut i = 0;
            vk.AcquireNextImageKHR(dev, self.swapchain, 60, 0, 0, &mut i);
            i
        };
        Frame {
            window: self,
            target_id: index,
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        xcb::unmap_window(&self.connection, self.window);
        xcb::destroy_window(&self.connection, self.window);
        self.connection.flush();
    }
}

pub fn init_xcb(title: &str, width: u32, height: u32) -> (Window, gfx_device_vulkan::Factory) {
    let (device, mut factory, backend) = gfx_device_vulkan::create(title, 1, &[],
        &["VK_KHR_surface", "VK_KHR_xcb_surface"], &["VK_KHR_swapchain"]);

    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();
    let (window, foreground) = {
        let setup = conn.get_setup();
        let screen = setup.roots().nth(screen_num as usize).unwrap();

        let foreground = conn.generate_id();
        xcb::create_gc(&conn, foreground, screen.root(), &[
                (xcb::GC_FOREGROUND, screen.black_pixel()),
                (xcb::GC_GRAPHICS_EXPOSURES, 0),
        ]);

        let win = conn.generate_id();
        xcb::create_window(&conn,
            xcb::COPY_FROM_PARENT as u8,
            win,
            screen.root(),
            0, 0,
            width as u16, height as u16,
            10,
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            screen.root_visual(), &[
                (xcb::CW_BACK_PIXEL, screen.black_pixel()),
                (xcb::CW_EVENT_MASK, xcb::EVENT_MASK_KEY_PRESS | xcb::EVENT_MASK_EXPOSURE),
            ]
        );
        (win, foreground)
    };

    xcb::map_window(&conn, window);
    xcb::change_property(&conn, xcb::PROP_MODE_REPLACE as u8, window,
        xcb::ATOM_WM_NAME, xcb::ATOM_STRING, 8, title.as_bytes());
    conn.flush();

    let surface = {
        let (inst, vk) = backend.get_instance();
        let info = vk::XcbSurfaceCreateInfoKHR {
            sType: vk::STRUCTURE_TYPE_XCB_SURFACE_CREATE_INFO_KHR,
            pNext: ptr::null(),
            flags: 0,
            connection: conn.get_raw_conn() as *const _,
            window: window as *const _, //HACK! TODO: fix the bindings
        };

        unsafe {
            let mut out = mem::zeroed();
            assert_eq!(vk::SUCCESS, vk.CreateXcbSurfaceKHR(inst, &info, ptr::null(), &mut out));
            out
        }
    };

    let (dev, vk) = backend.get_device();
    let mut images: [vk::Image; 2] = [0; 2];
    let mut num = images.len() as u32;

    let swapchain_info = vk::SwapchainCreateInfoKHR {
        sType: vk::STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR,
        pNext: ptr::null(),
        flags: 0,
        surface: surface,
        minImageCount: num,
        imageFormat: vk::FORMAT_R8G8B8A8_UNORM,
        imageColorSpace: vk::COLORSPACE_SRGB_NONLINEAR_KHR,
        imageExtent: vk::Extent2D { width: width, height: height },
        imageArrayLayers: 1,
        imageUsage: vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT | vk::IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT,
        imageSharingMode: vk::SHARING_MODE_EXCLUSIVE,
        queueFamilyIndexCount: 1,
        pQueueFamilyIndices: &0,
        preTransform: vk::SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
        compositeAlpha: vk::COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
        presentMode: vk::PRESENT_MODE_FIFO_RELAXED_KHR,
        clipped: vk::TRUE,
        oldSwapchain: 0,
    };

    let swapchain = unsafe {
        let mut out = mem::zeroed();
        assert_eq!(vk::SUCCESS, vk.CreateSwapchainKHR(dev, &swapchain_info, ptr::null(), &mut out));
        out
    };

    assert_eq!(vk::SUCCESS, unsafe {
        vk.GetSwapchainImagesKHR(dev, swapchain, &mut num, images.as_mut_ptr())
    });

    let format = format::Format(format::SurfaceType::R8_G8_B8_A8, format::ChannelType::Unorm);
    let targets = images[.. num as usize].iter().map(|image| {
        use gfx_core::factory::Typed;
        let raw_view = factory.view_swapchain_image(*image, format, (width, height)).unwrap();
        SwapTarget {
            _image: *image,
            target: Typed::new(raw_view),
            _fence: factory.create_fence(true),
        }
    }).collect();

    let win = Window {
        connection: conn,
        _foreground: foreground,
        window: window,
        swapchain: swapchain,
        targets: targets,
        queue: device,
    };
    (win, factory)
}
