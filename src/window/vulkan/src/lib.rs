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
extern crate vk_sys as vk;
extern crate gfx_core as core;
extern crate gfx_device_vulkan as device_vulkan;

#[cfg(target_os = "windows")]
extern crate kernel32;

use std::ffi::CStr;
use std::ptr;
use std::os::raw;
use core::format;
use core::memory::Typed;

#[cfg(unix)]
use winit::os::unix::WindowExt;
#[cfg(target_os = "windows")]
use winit::os::windows::WindowExt;

pub type TargetHandle<T> = core::handle::RenderTargetView<device_vulkan::Resources, T>;

pub struct SwapTarget<T> {
    _image: vk::Image,
    target: TargetHandle<T>,
    _fence: vk::Fence,
}

pub struct Window<T> {
    window: winit::Window,
    _debug_callback: Option<vk::DebugReportCallbackEXT>,
    swapchain: vk::SwapchainKHR,
    targets: Vec<SwapTarget<T>>,
    queue: device_vulkan::GraphicsQueue,
}

pub struct Frame<'a, T: 'a> {
    window: &'a mut Window<T>,
    target_id: u32,
}

impl<'a, T: Clone> Frame<'a, T> {
    pub fn get_target(&self) -> TargetHandle<T> {
        self.window.targets[self.target_id as usize].target.clone()
    }
    pub fn get_queue(&mut self) -> &mut device_vulkan::GraphicsQueue {
        &mut self.window.queue
    }
}

impl<'a, T> Drop for Frame<'a, T> {
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

impl<T: Clone> Window<T> {
    pub fn start_frame(&mut self) -> Frame<T> {
        //TODO: handle window resize (requires swapchain recreation)
        let index = unsafe {
            let (dev, vk) = self.queue.get_share().get_device();
            let mut i = 0;
            assert_eq!(vk::SUCCESS, vk.AcquireNextImageKHR(dev, self.swapchain, 60, 0, 0, &mut i));
            i
        };
        Frame {
            window: self,
            target_id: index,
        }
    }

    pub fn get_any_target(&self) -> TargetHandle<T> {
        self.targets[0].target.clone()
    }

    pub fn get_window(&mut self) -> &mut winit::Window {
        &mut self.window
    }

    pub fn get_size(&self) -> (u32, u32) {
        self.window.get_inner_size_points().unwrap()
    }
}

const LAYERS: &'static [&'static str] = &[
];
const LAYERS_DEBUG: &'static [&'static str] = &[
    "VK_LAYER_LUNARG_standard_validation",
];
const EXTENSIONS: &'static [&'static str] = &[
    "VK_KHR_surface",
];
const EXTENSIONS_DEBUG: &'static [&'static str] = &[
    "VK_KHR_surface",
    "VK_EXT_debug_report",
];
const DEV_EXTENSIONS: &'static [&'static str] = &[
    "VK_KHR_swapchain",
];

extern "system" fn callback(flags: vk::DebugReportFlagsEXT,
                            _ob_type: vk::DebugReportObjectTypeEXT, _object: u64, _location: usize,
                            _msg_code: i32, layer_prefix_c: *const raw::c_char,
                            description_c: *const raw::c_char, _user_data: *mut raw::c_void) -> u32
{
    let layer_prefix = unsafe { CStr::from_ptr(layer_prefix_c) }.to_str().unwrap();
    let description  = unsafe { CStr::from_ptr(description_c)  }.to_str().unwrap();
    println!("Vk flags {:x} in layer {}: {}", flags, layer_prefix, description);
    vk::FALSE
}

pub fn init<T: core::format::RenderFormat>(wb: winit::WindowBuilder, events_loop: &winit::EventsLoop)
                -> (Window<T>, device_vulkan::Factory) {
    let title = wb.window.title.clone();
    let window = wb.build(events_loop).unwrap();

    let debug = false;
    let (mut device, mut factory, backend) = device_vulkan::create(&title, 1,
        if debug {LAYERS_DEBUG} else {LAYERS},
        if debug {EXTENSIONS_DEBUG} else {EXTENSIONS},
        DEV_EXTENSIONS);

    let debug_callback = if debug {
        let info = vk::DebugReportCallbackCreateInfoEXT {
            sType: vk::STRUCTURE_TYPE_DEBUG_REPORT_CREATE_INFO_EXT,
            pNext: ptr::null(),
            flags: vk::DEBUG_REPORT_INFORMATION_BIT_EXT | vk::DEBUG_REPORT_WARNING_BIT_EXT |
                   vk::DEBUG_REPORT_PERFORMANCE_WARNING_BIT_EXT | vk::DEBUG_REPORT_ERROR_BIT_EXT |
                   vk::DEBUG_REPORT_DEBUG_BIT_EXT,
            pfnCallback: callback,
            pUserData: ptr::null_mut(),
        };
        let (inst, vk) = backend.get_instance();
        let mut out = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.CreateDebugReportCallbackEXT(inst, &info, ptr::null(), &mut out)
        });
        Some(out)
    }else {
        None
    };

    let surface = create_surface(backend.clone(), &window);

    let (dev, vk) = backend.get_device();
    let mut images: [vk::Image; 2] = [0; 2];
    let mut num = images.len() as u32;
    let format = <T as format::Formatted>::get_format();

    let surface_capabilities = {
        let (_, vk) = backend.get_instance();
        let dev = backend.get_physical_device();
        let mut capabilities: vk::SurfaceCapabilitiesKHR = unsafe { std::mem::uninitialized() };
        assert_eq!(vk::SUCCESS, unsafe {
            vk.GetPhysicalDeviceSurfaceCapabilitiesKHR(dev, surface, &mut capabilities)
        });
        capabilities
    };

    // Determine whether a queue family of a physical device supports presentation to a given surface 
    let supports_presentation = {
        let (_, vk) = backend.get_instance();
        let dev = backend.get_physical_device();
        let mut supported = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.GetPhysicalDeviceSurfaceSupportKHR(dev, device.get_family(), surface, &mut supported)
        });
        supported != 0
    };

    let surface_formats = {
        let (_, vk) = backend.get_instance();
        let dev = backend.get_physical_device();
        let mut num = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.GetPhysicalDeviceSurfaceFormatsKHR(dev, surface, &mut num, ptr::null_mut())
        });
        let mut formats = Vec::with_capacity(num as usize);
        assert_eq!(vk::SUCCESS, unsafe {
            vk.GetPhysicalDeviceSurfaceFormatsKHR(dev, surface, &mut num, formats.as_mut_ptr())
        });
        unsafe { formats.set_len(num as usize); }
        formats
    };

    let present_modes = {
        let (_, vk) = backend.get_instance();
        let dev = backend.get_physical_device();
        let mut num = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            vk.GetPhysicalDeviceSurfacePresentModesKHR(dev, surface, &mut num, ptr::null_mut())
        });
        let mut modes = Vec::with_capacity(num as usize);
        assert_eq!(vk::SUCCESS, unsafe {
            vk.GetPhysicalDeviceSurfacePresentModesKHR(dev, surface, &mut num, modes.as_mut_ptr())
        });
        unsafe { modes.set_len(num as usize); }
        modes
    };

    let (width, height) = window.get_inner_size_points().unwrap();

    // TODO: Use the queried information to check if our values are supported before creating the swapchain
    let swapchain_info = vk::SwapchainCreateInfoKHR {
        sType: vk::STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR,
        pNext: ptr::null(),
        flags: 0,
        surface: surface,
        minImageCount: num,
        imageFormat: device_vulkan::data::map_format(format.0, format.1).unwrap(),
        imageColorSpace: vk::COLOR_SPACE_SRGB_NONLINEAR_KHR,
        imageExtent: vk::Extent2D { width: width, height: height },
        imageArrayLayers: 1,
        imageUsage: vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
        imageSharingMode: vk::SHARING_MODE_EXCLUSIVE,
        queueFamilyIndexCount: 1,
        pQueueFamilyIndices: &0,
        preTransform: vk::SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
        compositeAlpha: vk::COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
        presentMode: vk::PRESENT_MODE_FIFO_KHR, // required to be supported
        clipped: vk::TRUE,
        oldSwapchain: 0,
    };

    let mut swapchain = 0;
    assert_eq!(vk::SUCCESS, unsafe {
        vk.CreateSwapchainKHR(dev, &swapchain_info, ptr::null(), &mut swapchain)
    });

    assert_eq!(vk::SUCCESS, unsafe {
        vk.GetSwapchainImagesKHR(dev, swapchain, &mut num, images.as_mut_ptr())
    });

    let mut cbuf = factory.create_command_buffer();

    let targets = images[.. num as usize].iter().map(|image| {
        cbuf.image_barrier(*image, vk::IMAGE_ASPECT_COLOR_BIT, vk::IMAGE_LAYOUT_UNDEFINED, vk::IMAGE_LAYOUT_PRESENT_SRC_KHR);
        let raw_view = factory.view_swapchain_image(*image, format, (width, height)).unwrap();
        SwapTarget {
            _image: *image,
            target: Typed::new(raw_view),
            _fence: factory.create_fence(true),
        }
    }).collect();

    {
        use core::Device;
        device.submit(&mut cbuf, &core::command::AccessInfo::new()).unwrap();
    }

    let win = Window {
        window: window,
        _debug_callback: debug_callback,
        swapchain: swapchain,
        targets: targets,
        queue: device,
    };
    (win, factory)
}

#[cfg(target_os = "windows")]
fn create_surface(backend: device_vulkan::SharePointer, window: &winit::Window) -> vk::SurfaceKHR {
    let (inst, vk) = backend.get_instance();
    let info = vk::Win32SurfaceCreateInfoKHR {
        sType: vk::STRUCTURE_TYPE_WIN32_SURFACE_CREATE_INFO_KHR,
        pNext: ptr::null(),
        flags: 0,
        hinstance: unsafe { kernel32::GetModuleHandleW(ptr::null()) } as *mut _,
        hwnd: window.get_hwnd() as *mut _,
    };
    let mut out = 0;
    assert_eq!(vk::SUCCESS, unsafe {
        vk.CreateWin32SurfaceKHR(inst, &info, ptr::null(), &mut out)
    });
    out
}

#[cfg(unix)]
fn create_surface(backend: device_vulkan::SharePointer, window: &winit::Window) -> vk::SurfaceKHR {
    let (inst, vk) = backend.get_instance();
    let info = vk::XcbSurfaceCreateInfoKHR {
        sType: vk::STRUCTURE_TYPE_XCB_SURFACE_CREATE_INFO_KHR,
        pNext: ptr::null(),
        flags: 0,
        connection: window.get_xcb_connection().unwrap() as *const _,
        window: window.get_xlib_window().unwrap() as *const _,
    };
    let mut out = 0;
    assert_eq!(vk::SUCCESS, unsafe {
        vk.CreateXcbSurfaceKHR(inst, &info, ptr::null(), &mut out)
    });
    out
}
