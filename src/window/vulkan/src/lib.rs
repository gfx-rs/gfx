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
extern crate gfx_core;
extern crate gfx_device_vulkan;

use std::{mem, ptr};
use gfx_device_vulkan::vk;

pub struct Window {
    pub backend: gfx_device_vulkan::Backend,
    pub win: winit::Window,
}

pub fn init(builder: winit::WindowBuilder) -> (Window, gfx_device_vulkan::command::GraphicsQueue) {
    //use winit::os::unix::WindowExt;
    let (backend, device) = gfx_device_vulkan::create(&builder.window.title, 1, &[],
        &["VK_KHR_surface", "VK_KHR_xcb_surface"], &["VK_KHR_swapchain"]);
    let (width, height) = builder.window.dimensions.unwrap_or((640, 400));

    if false {
        let surface = {
            let vk = backend.inst_pointers();
            let info = vk::XcbSurfaceCreateInfoKHR   {
                sType: vk::STRUCTURE_TYPE_XCB_SURFACE_CREATE_INFO_KHR,
                pNext: ptr::null(),
                flags: 0,
                connection: ptr::null_mut(), //TODO
                window: ptr::null_mut(), //TODO
            };

            unsafe {
                let mut out = mem::zeroed();
                let status = vk.CreateXcbSurfaceKHR(backend.instance(), &info, ptr::null(), &mut out);
                if status != vk::SUCCESS {
                    panic!("vkCreateXcbSurfaceKHR: {:?}", gfx_device_vulkan::Error(status));
                }
                out
            }
        };

        let vk = device.get_functions();
        let mut images: [vk::Image; 2] = [0; 2];
        let mut num = images.len() as u32;

        let info = vk::SwapchainCreateInfoKHR {
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
            let status = vk.CreateSwapchainKHR(backend.device(), &info, ptr::null(), &mut out);
            if status != vk::SUCCESS {
                panic!("vkCreateSwapchainKHR: {:?}", gfx_device_vulkan::Error(status));
            }
            out
        };

        let status = unsafe {
            vk.GetSwapchainImagesKHR(backend.device(), swapchain, &mut num, images.as_mut_ptr())
        };
        if status != vk::SUCCESS {
            panic!("vkGetSwapchainImagesKHR: {:?}", gfx_device_vulkan::Error(status));
        }
    }

    let win = Window {
        backend: backend,
        win: builder.build().unwrap(),
    };
    (win, device)
}
