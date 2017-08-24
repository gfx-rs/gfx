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
use std::borrow::Borrow;
use core::{format, handle};
use core::FrameSync;
use core::memory::Typed;
use device_vulkan::{data, native, CommandQueue, QueueFamily, VK_ENTRY, Instance};

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

pub struct RawSurface {
    instance: Arc<Instance>,
    pub handle: vk::SurfaceKHR,
    pub loader: vk::SurfaceFn,
}

impl Drop for RawSurface {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface_khr(self.instance.raw.handle(), self.handle, ptr::null()); }
    }
}

impl Surface {
    fn from_raw(instance: Arc<Instance>, surface: vk::SurfaceKHR, (width, height): (u32, u32)) -> Surface {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");

        let loader = vk::SurfaceFn::load(|name| {
                unsafe {
                    mem::transmute(entry.get_instance_proc_addr(
                        instance.raw.handle(),
                        name.as_ptr()))
                }
            }).expect("Unable to load surface functions");

        let raw = Arc::new(RawSurface {
            instance: instance,
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
    fn from_window(window: &winit::Window, instance: Arc<Instance>) -> Surface {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");

        let surface = instance.surface_extensions.iter().map(|&extension| {
            match extension {
                vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME => {
                    use winit::os::unix::WindowExt;
                    let xlib_loader = if let Ok(loader) = ash::extensions::XlibSurface::new(entry, &instance.raw) {
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

        Surface::from_raw(instance, surface, window.get_inner_size_pixels().unwrap())
    }

    #[cfg(target_os = "windows")]
    fn from_window(window: &winit::Window, instance: Arc<Instance>) -> Surface {
        use winit::os::windows::WindowExt;
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");
        let win32_loader = ash::extensions::Win32Surface::new(entry, &instance.raw)
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

        Surface::from_raw(instance, surface, window.get_inner_size_pixels().unwrap())
    }
}

impl core::Surface<device_vulkan::Backend> for Surface {
    type Swapchain = Swapchain;

    fn supports_queue(&self, queue_family: &device_vulkan::QueueFamily) -> bool {
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

    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, present_queue: &Q) -> Self::Swapchain
        where Q: AsRef<device_vulkan::CommandQueue>
    {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");
        let loader = vk::SwapchainFn::load(|name| {
                unsafe {
                    mem::transmute(entry.get_instance_proc_addr(
                        self.raw.instance.raw.handle(),
                        name.as_ptr()))
                }
            }).expect("Unable to load swapchain functions");

        // TODO: check for better ones if available
        let present_mode = vk::PresentModeKHR::Fifo; // required to be supported
        let present_queue = present_queue.as_ref();

        // TODO: handle depth stencil
        let format = config.color_format;

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
            assert_eq!(vk::Result::Success,
                loader.create_swapchain_khr(
                    present_queue.device_handle(),
                    &info,
                    ptr::null(),
                    &mut swapchain));
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

        Swapchain::from_raw(
            swapchain,
            present_queue,
            loader,
            swapchain_images)
    }
}

pub struct Swapchain {
    raw: vk::SwapchainKHR,
    device: Arc<device_vulkan::RawDevice>,
    swapchain_fn: vk::SwapchainFn,
    images: Vec<native::Image>,

    // Queued up frames for presentation
    frame_queue: VecDeque<usize>,
}

impl Swapchain {
    fn from_raw(raw: vk::SwapchainKHR,
                queue: &CommandQueue,
                swapchain_fn: vk::SwapchainFn,
                images: Vec<native::Image>) -> Self
    {
        Swapchain {
            raw: raw,
            device: queue.device(),
            swapchain_fn: swapchain_fn,
            images: images,
            frame_queue: VecDeque::new(),
        }
    }
}

impl core::Swapchain<device_vulkan::Backend> for Swapchain {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<device_vulkan::Backend>] {
        // TODO
        unimplemented!()
    }

    fn acquire_frame(&mut self, sync: FrameSync<device_vulkan::Resources>) -> core::Frame {
        let index = {
            let acquire = |semaphore, fence| {
                unsafe {
                    let mut index = mem::uninitialized();
                    self.swapchain_fn.acquire_next_image_khr(
                            self.device.0.handle(),
                            self.raw,
                            std::u64::MAX, // will block if no image is available
                            semaphore,
                            fence,
                            &mut index);
                    index
                }
            };

            let mut manager = handle::Manager::new();

            match sync {
                FrameSync::Semaphore(semaphore) => {
                    acquire(manager.ref_semaphore(semaphore).lock().unwrap().0, vk::Fence::null())
                }
                FrameSync::Fence(fence) => {
                    acquire(vk::Semaphore::null(), manager.ref_fence(fence).lock().unwrap().0)
                }
            }
        };

        self.frame_queue.push_back(index as usize);
        unsafe { core::Frame::new(index as usize) }
    }

    fn present<Q>(&mut self, present_queue: &mut Q, wait_semaphores: &[&handle::Semaphore<device_vulkan::Resources>])
        where Q: AsMut<device_vulkan::CommandQueue>
    {
        let frame = self.frame_queue.pop_front().expect("No frame currently queued up. Need to acquire a frame first.");

        // TODO: wait semaphores
        let info = vk::PresentInfoKHR {
            s_type: vk::StructureType::PresentInfoKhr,
            p_next: ptr::null(),
            wait_semaphore_count: 0,
            p_wait_semaphores: ptr::null(),
            swapchain_count: 1,
            p_swapchains: &self.raw,
            p_image_indices: &(frame as u32),
            p_results: ptr::null_mut(),
        };

        unsafe {
            self.swapchain_fn.queue_present_khr(*present_queue.as_mut().raw(), &info);
        }
        // TODO: handle result and return code
    }
}

pub struct Window(pub winit::Window);

impl core::WindowExt<device_vulkan::Backend> for Window {
    type Surface = Surface;
    type Adapter = device_vulkan::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface, Vec<device_vulkan::Adapter>) {
        let instance = Arc::new(Instance::create());
        let surface = Surface::from_window(&self.0, instance.clone());
        let adapters = Instance::enumerate_adapters(instance);
        (surface, adapters)
    }
}
