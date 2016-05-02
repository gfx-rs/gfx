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

extern crate shared_library;
extern crate gfx_core;

use std::{fmt, mem, ptr};
use shared_library::dynamic_library::DynamicLibrary;

mod vk {
    #![allow(dead_code)]
    #![allow(non_upper_case_globals)]
    #![allow(non_snake_case)]
    #![allow(non_camel_case_types)]
    include!(concat!(env!("OUT_DIR"), "/vk_bindings.rs"));
}


struct PhysicalDeviceInfo {
    device: vk::PhysicalDevice,
    properties: vk::PhysicalDeviceProperties,
    queue_families: Vec<vk::QueueFamilyProperties>,
    memory: vk::PhysicalDeviceMemoryProperties,
    features: vk::PhysicalDeviceFeatures,
}

impl PhysicalDeviceInfo {
    pub fn new(dev: vk::PhysicalDevice, vk: &vk::InstancePointers) -> PhysicalDeviceInfo {
        PhysicalDeviceInfo {
            device: dev,
            properties: unsafe {
                let mut out = mem::zeroed();
                vk.GetPhysicalDeviceProperties(dev, &mut out);
                out
            },
            queue_families: unsafe {
                let mut num = 4;
                let mut families = Vec::with_capacity(num as usize);
                vk.GetPhysicalDeviceQueueFamilyProperties(dev, &mut num, families.as_mut_ptr());
                families.set_len(num as usize);
                families
            },
            memory: unsafe {
                let mut out = mem::zeroed();
                vk.GetPhysicalDeviceMemoryProperties(dev, &mut out);
                out
            },
            features: unsafe {
                let mut out = mem::zeroed();
                vk.GetPhysicalDeviceFeatures(dev, &mut out);
                out
            },
        }
    }
}

pub struct Backend {
    dynamic_lib: DynamicLibrary,
    library: vk::Static,
    instance: vk::Instance,
    inst_pointers: vk::InstancePointers,
    functions: vk::EntryPoints,
    device: vk::Device,
}

pub fn create(app_name: &str, app_version: u32, layers: &[&str], extensions: &[&str]) -> Backend {
    use std::ffi::CString;
    use std::path::Path;

    let dynamic_lib = DynamicLibrary::open(Some(Path::new("libvulkan.so"))).unwrap();
    let lib = vk::Static::load(|name| unsafe {
        let name = name.to_str().unwrap();
        dynamic_lib.symbol(name).unwrap()
    });
    let entry_points = vk::EntryPoints::load(|name| unsafe {
        mem::transmute(lib.GetInstanceProcAddr(0, name.as_ptr()))
    });

    let app_info = vk::ApplicationInfo {
        sType: vk::STRUCTURE_TYPE_APPLICATION_INFO,
        pNext: ptr::null(),
        pApplicationName: app_name.as_ptr() as *const i8,
        applicationVersion: app_version,
        pEngineName: "gfx-rs".as_ptr() as *const i8,
        engineVersion: 0x1000, //TODO
        apiVersion: 0x400000, //TODO
    };

    let cstrings = layers.iter().chain(extensions.iter())
                         .map(|&s| CString::new(s).unwrap())
                         .collect::<Vec<_>>();
    let str_pointers = cstrings.iter().map(|s| s.as_ptr())
                               .collect::<Vec<_>>();

    let create_info = vk::InstanceCreateInfo {
        sType: vk::STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
        pNext: ptr::null(),
        flags: 0,
        pApplicationInfo: &app_info,
        enabledLayerCount: layers.len() as u32,
        ppEnabledLayerNames: str_pointers.as_ptr(),
        enabledExtensionCount: extensions.len() as u32,
        ppEnabledExtensionNames: str_pointers[layers.len()..].as_ptr(),
    };

    let instance = unsafe {
        let mut ptr = mem::zeroed();
        let status = entry_points.CreateInstance(&create_info, ptr::null(), &mut ptr);
        if status != vk::SUCCESS {
            panic!("vkCreateInstance: {:?}", Error(status));
        }
        ptr
    };

    let inst_pointers = vk::InstancePointers::load(|name| unsafe {
        mem::transmute(lib.GetInstanceProcAddr(instance, name.as_ptr()))
    });

    let mut physical_devices: [vk::PhysicalDevice; 4] = unsafe { mem::zeroed() };
    let mut num = physical_devices.len() as u32;
    let status = unsafe {
        inst_pointers.EnumeratePhysicalDevices(instance, &mut num, physical_devices.as_mut_ptr())
    };
    if status != vk::SUCCESS {
        panic!("vkEnumeratePhysicalDevices: {:?}", Error(status));
    }
    let devices = physical_devices[..num as usize].iter()
        .map(|dev| PhysicalDeviceInfo::new(*dev, &inst_pointers))
        .collect::<Vec<_>>();

    let device = {
        let physical = devices[0].device;
        let ext = CString::new("VK_KHR_swapchain").unwrap();
        let queue_info = vk::DeviceQueueCreateInfo {
            sType: vk::STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            queueFamilyIndex: 0,
            queueCount: 1,
            pQueuePriorities: &1.0,
        };
        let features = unsafe{ mem::zeroed() };
        let dev_info = vk::DeviceCreateInfo {
            sType: vk::STRUCTURE_TYPE_DEVICE_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            queueCreateInfoCount: 1,
            pQueueCreateInfos: &queue_info,
            enabledLayerCount: 0,
            ppEnabledLayerNames: ptr::null(),
            enabledExtensionCount: 1,
            ppEnabledExtensionNames: &ext.as_ptr(),
            pEnabledFeatures: &features,
        };
        unsafe {
            let mut out = mem::zeroed();
            let status = inst_pointers.CreateDevice(physical, &dev_info, ptr::null(), &mut out);
            if status != vk::SUCCESS {
                panic!("vkCreateDevice: {:?}", Error(status));
            }
            out
        }
    };

    Backend {
        dynamic_lib: dynamic_lib,
        library: lib,
        instance: instance,
        inst_pointers: inst_pointers,
        functions: entry_points,
        device: device,
    }
}


#[derive(Clone, PartialEq, Eq)]
pub struct Error(vk::Result);

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self.0 {
            vk::SUCCESS => "success",
            vk::NOT_READY => "not ready",
            vk::TIMEOUT => "timeout",
            vk::EVENT_SET => "event_set",
            vk::EVENT_RESET => "event_reset",
            vk::INCOMPLETE => "incomplete",
            vk::ERROR_OUT_OF_HOST_MEMORY => "out of host memory",
            vk::ERROR_OUT_OF_DEVICE_MEMORY => "out of device memory",
            vk::ERROR_INITIALIZATION_FAILED => "initialization failed",
            vk::ERROR_DEVICE_LOST => "device lost",
            vk::ERROR_MEMORY_MAP_FAILED => "memory map failed",
            vk::ERROR_LAYER_NOT_PRESENT => "layer not present",
            vk::ERROR_EXTENSION_NOT_PRESENT => "extension not present",
            vk::ERROR_FEATURE_NOT_PRESENT => "feature not present",
            vk::ERROR_INCOMPATIBLE_DRIVER => "incompatible driver",
            vk::ERROR_TOO_MANY_OBJECTS => "too many objects",
            vk::ERROR_FORMAT_NOT_SUPPORTED => "format not supported",
            vk::ERROR_SURFACE_LOST_KHR => "surface lost (KHR)",
            vk::ERROR_NATIVE_WINDOW_IN_USE_KHR => "native window in use (KHR)",
            vk::SUBOPTIMAL_KHR => "suboptimal (KHR)",
            vk::ERROR_OUT_OF_DATE_KHR => "out of date (KHR)",
            vk::ERROR_INCOMPATIBLE_DISPLAY_KHR => "incompatible display (KHR)",
            vk::ERROR_VALIDATION_FAILED_EXT => "validation failed (EXT)",
            _ => "unknown",
        })
    }
}