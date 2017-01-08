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

extern crate ash;
extern crate gfx_corell as core;
extern crate kernel32;
#[macro_use]
extern crate lazy_static;
extern crate winit;

use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0, V1_0};
use ash::vk;
use ash::{Entry, LoadingError};

use std::ffi::{CStr, CString};
use std::iter;
use std::mem;
use std::ptr;
use std::sync::Arc;

lazy_static! {
    static ref VK_ENTRY: Result<Entry<V1_0>, LoadingError> = Entry::new();
}

pub struct PhysicalDevice {
    handle: vk::PhysicalDevice,
    queue_families: Vec<vk::QueueFamilyProperties>,
    info: core::PhysicalDeviceInfo,
    instance: Arc<InstanceInner>,
}

impl core::PhysicalDevice for PhysicalDevice {
    type B = Backend;

    fn open(&self) -> (Device, Vec<CommandQueue>) {
        let queue_infos = self.queue_families.iter()
            .enumerate()
            .map(|(i, queue_family)| {
                vk::DeviceQueueCreateInfo {
                    s_type: vk::StructureType::DeviceQueueCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::DeviceQueueCreateFlags::empty(),
                    queue_family_index: i as u32,
                    queue_count: queue_family.queue_count,
                    p_queue_priorities: &1.0,
                }
            }).collect::<Vec<_>>();

        // Create device
        let device_extensions = &["VK_KHR_swapchain"];

        let device = {
            let cstrings = device_extensions.iter()
                                    .map(|&s| CString::new(s).unwrap())
                                    .collect::<Vec<_>>();

            let str_pointers = cstrings.iter()
                                    .map(|s| s.as_ptr())
                                    .collect::<Vec<_>>();

            let features = unsafe { mem::zeroed() };
            let info = vk::DeviceCreateInfo {
                s_type: vk::StructureType::DeviceCreateInfo,
                p_next: ptr::null(),
                flags: vk::DeviceCreateFlags::empty(),
                queue_create_info_count: queue_infos.len() as u32,
                p_queue_create_infos: queue_infos.as_ptr(),
                enabled_layer_count: 0,
                pp_enabled_layer_names: ptr::null(),
                enabled_extension_count: str_pointers.len() as u32,
                pp_enabled_extension_names: str_pointers.as_ptr(),
                p_enabled_features: &features,
            };

            unsafe {
                self.instance.0.create_device(self.handle, &info, None)
                    .expect("Error on device creation")
            }
        };

        // Create associated command queues
        let queues = queue_infos.iter().flat_map(|info| {
                (0..info.queue_count).map(|id| {
                    let queue = unsafe { device.get_device_queue(info.queue_family_index, id) };
                    CommandQueue { }
                }).collect::<Vec<_>>()
            }).collect();

        let device = Device {
            inner: Arc::new(DeviceInner(device)),
        };

        (device, queues)
    }

    fn get_info(&self) -> &core::PhysicalDeviceInfo {
        &self.info
    }
}

struct DeviceInner(ash::Device<V1_0>);
impl Drop for DeviceInner {
    fn drop(&mut self) {
        unsafe { self.0.destroy_device(None); }
    }
}

pub struct Device {
    inner: Arc<DeviceInner>,
}

impl core::Device for Device {

}

pub struct CommandQueue {

}

impl core::CommandQueue for CommandQueue {
    type B = Backend;

    fn submit(&mut self, cmd_buffer: &()) {
        unimplemented!()
    }
}

struct SurfaceInner {
    handle: vk::SurfaceKHR,
    instance: Arc<InstanceInner>,
    loader: vk::SurfaceFn,
}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface_khr(self.instance.0.handle(), self.handle, ptr::null()); }
    }
}

pub struct Surface {
    // Vk (EXT) specs [29.2.7 Platform-Independent Information]
    // For vkDestroySurfaceKHR: Host access to surface must be externally synchronized
    inner: Arc<SurfaceInner>,
}

impl Surface {
    fn from_raw(instance: &Instance, surface: vk::SurfaceKHR) -> Surface {
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");

        let loader = vk::SurfaceFn::load(|name| {
                unsafe {
                    mem::transmute(entry.get_instance_proc_addr(
                        instance.inner.0.handle(),
                        name.as_ptr()))
                }
            }).expect("Unable to load surface functions");

        let inner = Arc::new(SurfaceInner {
            handle: surface,
            instance: instance.inner.clone(),
            loader: loader,
        });

        Surface {
            inner: inner,
        }
    }
}

impl core::Surface for Surface {
    type B = Backend;
    type Window = winit::Window;

    #[cfg(target_os = "windows")]
    fn from_window(window: &winit::Window, instance: &Instance) -> Surface {
        use winit::os::windows::WindowExt;
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");
        let win32_loader = ash::extensions::Win32Surface::new(entry, &instance.inner.0)
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

        Self::from_raw(instance, surface)
    }

    #[cfg(not(target_os = "windows"))]
    fn from_window(window: &winit::Window, instance: &Instance) -> Surface {
        unimplemented!()
    }

    fn build_swapchain<T: core::format::RenderFormat>(
                    &self, width: u32, height: u32,
                    present_queue: &CommandQueue) -> SwapChain {
        SwapChain {

        }
    }
}

pub struct SwapChain {

}

impl core::SwapChain for SwapChain {
    type B = Backend;

    fn present(&mut self) {
        // unimplemented!()
    }
}

struct InstanceInner(pub ash::Instance<V1_0>);
impl Drop for InstanceInner {
    fn drop(&mut self) {
        unsafe { self.0.destroy_instance(None); }
    }
}

const SURFACE_EXTENSIONS: &'static [&'static str] = &[
    "VK_KHR_surface",

    // Platform-specific WSI extensions
    "VK_KHR_xlib_surface",
    "VK_KHR_xcb_surface",
    "VK_KHR_wayland_surface",
    "VK_KHR_mir_surface",
    "VK_KHR_android_surface",
    "VK_KHR_win32_surface",
];

pub struct Instance {
    // Vk specs [2.5 Threading Behavior]
    // Externally Synchronized Parameters: The `instance` parameter in `vkDestroyInstance`
    // `Arc` ensures that we only call drop once
    inner: Arc<InstanceInner>,
}

impl core::Instance for Instance {
    type B = Backend;

    fn create() -> Instance {
        // TODO: return errors instead of panic
        let entry = VK_ENTRY.as_ref().expect("Unable to load vulkan entry points");

        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::ApplicationInfo,
            p_next: ptr::null(),
            p_application_name: "vulkan_ll".as_ptr() as *const _, // TODO:
            application_version: 0,
            p_engine_name: "gfx-rs".as_ptr() as *const _,
            engine_version: 0, //TODO
            api_version: 0, //TODO
        };

        let instance_extensions = entry.enumerate_instance_extension_properties()
                                       .expect("Unable to enumerate instance extensions");
        // Check our surface extensions against the available extensions
        let surface_extensions = SURFACE_EXTENSIONS.iter().filter_map(|ext| {
            instance_extensions.iter().find(|inst_ext| {
                unsafe { CStr::from_ptr(inst_ext.extension_name.as_ptr()) == CStr::from_ptr(ext.as_ptr() as *const i8) }
            }).and_then(|_| Some(*ext))
        }).collect::<Vec<&str>>();

        let instance = {
            let cstrings = surface_extensions.iter()
                                    .map(|&s| CString::new(s).unwrap())
                                    .collect::<Vec<_>>();

            let str_pointers = cstrings.iter()
                                    .map(|s| s.as_ptr())
                                    .collect::<Vec<_>>();

            let create_info = vk::InstanceCreateInfo {
                s_type: vk::StructureType::InstanceCreateInfo,
                p_next: ptr::null(),
                flags: vk::InstanceCreateFlags::empty(),
                p_application_info: &app_info,
                enabled_layer_count: 0,
                pp_enabled_layer_names: ptr::null(),
                enabled_extension_count: str_pointers.len() as u32,
                pp_enabled_extension_names: str_pointers.as_ptr(),
            };

            entry.create_instance(&create_info, None).expect("Unable to create vulkan instance")
        };

        Instance {
            inner: Arc::new(InstanceInner(instance)),
        }
    }

    fn enumerate_physical_devices(&self) -> Vec<PhysicalDevice> {
        self.inner.0.enumerate_physical_devices()
            .expect("Unable to enumerate physical devices")
            .iter()
            .map(|&device| {
                // TODO: add an ash function for this
                let properties = unsafe {
                    let mut out = mem::zeroed();
                    self.inner.0.fp_v1_0().get_physical_device_properties(device, &mut out);
                    out
                };

                let info = core::PhysicalDeviceInfo {
                    name: String::new(), // TODO: retrieve name
                    vendor: properties.vendor_id as usize,
                    device: properties.device_id as usize,
                    software_rendering: properties.device_type == vk::PhysicalDeviceType::Cpu,
                };

                let queue_families = self.inner.0.get_physical_device_queue_family_properties(device);

                PhysicalDevice {
                    handle: device,
                    queue_families: queue_families,
                    info: info,
                    instance: self.inner.clone(),
                }
            })
            .collect()
    }
}

pub enum Backend { }

impl core::Backend for Backend {
    type CommandBuffer = ();
    type CommandQueue = CommandQueue;
    type Device = Device;
    type Instance = Instance;
    type PhysicalDevice = PhysicalDevice;
    type Resources = Resources;
    type Surface = Surface;
    type SwapChain = SwapChain;
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources { }

impl core::Resources for Resources {
    type Buffer = ();
    type Shader = ();
    type RenderPass = ();
    type PipelineLayout = ();
    type PipelineStateObject = ();
    type Image = ();
    type ShaderResourceView = ();
    type UnorderedAccessView = ();
    type RenderTargetView = ();
    type DepthStencilView = ();
    type Sampler = ();
}
