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
extern crate ash;
extern crate gfx_core as core;
extern crate gfx_device_vulkan as device_vulkan;

#[cfg(target_os = "windows")]
extern crate kernel32;

use ash::vk;
use ash::version::{EntryV1_0, InstanceV1_0};
use std::collections::VecDeque;
use std::ffi::CStr;
use std::{mem, ptr};
use std::os::raw;
use std::sync::Arc;
use core::format;
use core::memory::Typed;
use device_vulkan::{data, native, CommandQueue, RawSurface, SwapChain, QueueFamily, VK_ENTRY, INSTANCE};

#[cfg(unix)]
use winit::os::unix::WindowExt;
#[cfg(target_os = "windows")]
use winit::os::windows::WindowExt;

pub struct Surface {
    // Vk (EXT) specs [29.2.7 Platform-Independent Information]
    // For vkDestroySurfaceKHR: Host access to surface must be externally synchronized
    raw: Arc<RawSurface>,
    width: u32,
    height: u32,
}

impl Surface {
    fn from_raw(surface: vk::SurfaceKHR, (width, height): (u32, u32)) -> Surface {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");

        let loader = vk::SurfaceFn::load(|name| {
                unsafe {
                    mem::transmute(entry.get_instance_proc_addr(
                        INSTANCE.raw.handle(),
                        name.as_ptr()))
                }
            }).expect("Unable to load surface functions");

        let raw = Arc::new(RawSurface {
            handle: surface,
            loader: loader,
        });

        Surface {
            raw: raw,
            width: width,
            height: height,
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn from_window(window: &winit::Window) -> Surface {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");

        let surface = self.surface_extensions.iter().map(|&extension| {
            match extension {
                vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME => {
                    use winit::os::unix::WindowExt;
                    let xlib_loader = if let Ok(loader) = ash::extensions::XlibSurface::new(entry, &INSTANCE.raw) {
                        loader
                    } else {
                        return None;
                    };

                    unsafe {
                        let info = vk::XlibSurfaceCreateInfoKHR {
                            s_type: vk::StructureType::XlibSurfaceCreateInfoKhr,
                            p_next: ptr::null(),
                            flags: vk::XlibSurfaceCreateFlagsKHR::empty(),
                            window: window.get_xlib_window().unwrap() as *const _,
                            dpy: window.get_xlib_display().unwrap() as *mut _,
                        };

                        xlib_loader.create_xlib_surface_khr(&info, None).ok()
                    }
                },
                // TODO: other platforms
                _ => None,
            }
        }).find(|x| x.is_some())
          .expect("Unable to find a surface implementation.")
          .unwrap();

        Surface::from_raw(surface, window.get_inner_size_pixels().unwrap())
    }

    #[cfg(target_os = "windows")]
    fn from_window(window: &winit::Window) -> Surface {
        use winit::os::windows::WindowExt;
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");
        let win32_loader = ash::extensions::Win32Surface::new(entry, &INSTANCE.raw)
                        .expect("Unable to load win32 surface functions");

        let surface = unsafe {
            let info = vk::Win32SurfaceCreateInfoKHR {
                s_type: vk::StructureType::Win32SurfaceCreateInfoKhr,
                p_next: ptr::null(),
                flags: vk::Win32SurfaceCreateFlagsKHR::empty(),
                hinstance: unsafe { kernel32::GetModuleHandleW(ptr::null()) } as *mut _,
                hwnd: window.get_hwnd() as *mut _,
            };

            win32_loader.create_win32_surface_khr(&info, None)
                .expect("Error on surface creation")
        };

        Surface::from_raw(surface, window.get_inner_size_pixels().unwrap())
    }
}

impl core::Surface for Surface {
    type CommandQueue = CommandQueue;
    type SwapChain = SwapChain;
    type QueueFamily = QueueFamily;

    fn supports_queue(&self, queue_family: &Self::QueueFamily) -> bool {
        unsafe {
            let mut support = mem::uninitialized();
            self.raw.loader.get_physical_device_surface_support_khr(
                queue_family.device(),
                queue_family.family_index(),
                self.raw.handle,
                &mut support);
            support == vk::VK_TRUE
        }
    }

    fn build_swapchain<T: core::format::RenderFormat>(&self,
                    present_queue: &CommandQueue) -> SwapChain {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");
        let loader = vk::SwapchainFn::load(|name| {
                unsafe {
                    mem::transmute(entry.get_instance_proc_addr(
                        INSTANCE.raw.handle(),
                        name.as_ptr()))
                }
            }).expect("Unable to load swapchain functions");

        // TODO: check for better ones if available
        let present_mode = vk::PresentModeKHR::Fifo; // required to be supported

        let format = <T as format::Formatted>::get_format();

        let info = vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SwapchainCreateInfoKhr,
            p_next: ptr::null(),
            flags: vk::SwapchainCreateFlagsKHR::empty(),
            surface: self.raw.handle,
            min_image_count: 2, // TODO: let the user specify the value
            image_format: data::map_format(format.0, format.1).unwrap(),
            image_color_space: vk::ColorSpaceKHR::SrgbNonlinear,
            image_extent: vk::Extent2D {
                width: self.width,
                height: self.height
            },
            image_array_layers: 1,
            image_usage: vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
            image_sharing_mode: vk::SharingMode::Exclusive,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            pre_transform: vk::SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
            composite_alpha: vk::COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
            present_mode: present_mode,
            clipped: 1,
            old_swapchain: vk::SwapchainKHR::null(), 
        };

        let swapchain = unsafe {
            let mut swapchain = mem::uninitialized();
            assert_eq!(vk::Result::Success, unsafe {
                loader.create_swapchain_khr(
                    present_queue.device_handle(),
                    &info,
                    ptr::null(),
                    &mut swapchain)
            });
            swapchain
        };

        let swapchain_images = unsafe {
            // TODO: error handling
            let mut count = 0;
            loader.get_swapchain_images_khr(
                present_queue.device_handle(),
                swapchain,
                &mut count,
                ptr::null_mut());

            let mut v = Vec::with_capacity(count as vk::size_t);
            loader.get_swapchain_images_khr(
                present_queue.device_handle(),
                swapchain,
                &mut count,
                v.as_mut_ptr());

            v.set_len(count as vk::size_t);
            v.into_iter().map(|image| native::Image(image))
                    .collect::<Vec<_>>()
        };

        SwapChain::from_raw(
            swapchain,
            &present_queue,
            loader,
            swapchain_images)
    }
}

pub struct Window<'a>(&'a winit::Window);

impl<'a> core::WindowExt for Window<'a> {
    type Surface = Surface;
    type Adapter = device_vulkan::Adapter;

    /// Create window surface and enumerate all available adapters.
    fn get_surface_and_adapters(&mut self) -> (Surface, Vec<device_vulkan::Adapter>) {
        let surface = Surface::from_window(self.0);
        let adapters = INSTANCE.raw.enumerate_physical_devices()
            .expect("Unable to enumerate adapter")
            .iter()
            .map(|&device| {
                let properties = INSTANCE.raw.get_physical_device_properties(device);
                let name = unsafe {
                    CStr::from_ptr(properties.device_name.as_ptr())
                            .to_str()
                            .expect("Invalid UTF-8 string")
                            .to_owned()
                };

                let info = core::AdapterInfo {
                    name: name,
                    vendor: properties.vendor_id as usize,
                    device: properties.device_id as usize,
                    software_rendering: properties.device_type == vk::PhysicalDeviceType::Cpu,
                };

                let queue_families = INSTANCE.raw.get_physical_device_queue_family_properties(device)
                                                 .iter()
                                                 .enumerate()
                                                 .map(|(i, queue_family)| {
                                                    QueueFamily::from_raw(
                                                        device,
                                                        i as u32,
                                                        queue_family,
                                                    )
                                                 }).collect();

                device_vulkan::Adapter::from_raw(device, queue_families, info)
            })
            .collect();

        (surface, adapters)
    }
}
