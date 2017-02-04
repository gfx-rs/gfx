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

#[macro_use]
extern crate log;
extern crate shared_library;
extern crate gfx_core as core;
extern crate vk_sys as vk;
extern crate spirv_utils;

use std::{fmt, iter, mem, ptr};
use std::sync::{Arc, Mutex};
use std::ffi::CStr;
use shared_library::dynamic_library::DynamicLibrary;

pub use self::command::{GraphicsQueue, Buffer as CommandBuffer};
pub use self::factory::Factory;

mod command;
pub mod data;
mod factory;
mod native;
mod mirror;

struct PhysicalDeviceInfo {
    device: vk::PhysicalDevice,
    _properties: vk::PhysicalDeviceProperties,
    queue_families: Vec<vk::QueueFamilyProperties>,
    memory: vk::PhysicalDeviceMemoryProperties,
    _features: vk::PhysicalDeviceFeatures,
}

impl PhysicalDeviceInfo {
    pub fn new(dev: vk::PhysicalDevice, vk: &vk::InstancePointers) -> PhysicalDeviceInfo {
        PhysicalDeviceInfo {
            device: dev,
            _properties: unsafe {
                let mut out = mem::zeroed();
                vk.GetPhysicalDeviceProperties(dev, &mut out);
                out
            },
            queue_families: unsafe {
                let mut num = 0;
                vk.GetPhysicalDeviceQueueFamilyProperties(dev, &mut num, ptr::null_mut());
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
            _features: unsafe {
                let mut out = mem::zeroed();
                vk.GetPhysicalDeviceFeatures(dev, &mut out);
                out
            },
        }
    }
}


pub struct Share {
    _dynamic_lib: DynamicLibrary,
    _library: vk::Static,
    instance: vk::Instance,
    inst_pointers: vk::InstancePointers,
    device: vk::Device,
    dev_pointers: vk::DevicePointers,
    physical_device: vk::PhysicalDevice,
    handles: Mutex<core::handle::Manager<Resources>>,
}

pub type SharePointer = Arc<Share>;

impl Share {
    pub fn get_instance(&self) -> (vk::Instance, &vk::InstancePointers) {
        (self.instance, &self.inst_pointers)
    }
    pub fn get_device(&self) -> (vk::Device, &vk::DevicePointers) {
        (self.device, &self.dev_pointers)
    }
    pub fn get_physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }
}

const SURFACE_EXTENSIONS: &'static [&'static str] = &[
    // Platform-specific WSI extensions
    "VK_KHR_xlib_surface",
    "VK_KHR_xcb_surface",
    "VK_KHR_wayland_surface",
    "VK_KHR_mir_surface",
    "VK_KHR_android_surface",
    "VK_KHR_win32_surface",
];


pub fn create(app_name: &str, app_version: u32, layers: &[&str], extensions: &[&str],
              dev_extensions: &[&str]) -> (command::GraphicsQueue, factory::Factory, SharePointer) {
    use std::ffi::CString;
    use std::path::Path;

    let dynamic_lib = DynamicLibrary::open(Some(
            if cfg!(target_os = "windows") {
                Path::new("vulkan-1.dll")
            } else {
                Path::new("libvulkan.so.1")
            }
        )).expect("Unable to open vulkan shared library");
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
        pApplicationName: app_name.as_ptr() as *const _,
        applicationVersion: app_version,
        pEngineName: "gfx-rs".as_ptr() as *const _,
        engineVersion: 0x1000, //TODO
        apiVersion: 0x400000, //TODO
    };

    let instance_extensions = {
        let mut num = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            entry_points.EnumerateInstanceExtensionProperties(ptr::null(), &mut num, ptr::null_mut())
        });
        let mut out = Vec::with_capacity(num as usize);
        assert_eq!(vk::SUCCESS, unsafe {
            entry_points.EnumerateInstanceExtensionProperties(ptr::null(), &mut num, out.as_mut_ptr())
        });
        unsafe { out.set_len(num as usize); }
        out
    };

    // Check our surface extensions against the available extensions
    let surface_extensions = SURFACE_EXTENSIONS.iter().filter_map(|ext| {
        instance_extensions.iter().find(|inst_ext| {
            unsafe { CStr::from_ptr(inst_ext.extensionName.as_ptr()) == CStr::from_ptr(ext.as_ptr() as *const i8) }
        }).and_then(|_| Some(*ext))
    }).collect::<Vec<&str>>();
    
    let instance = {
        let cstrings = layers.iter().chain(extensions.iter())
                                    .chain(surface_extensions.iter())
                         .map(|&s| CString::new(s).unwrap())
                         .collect::<Vec<_>>();
        let str_pointers = cstrings.iter()
                                   .map(|s| s.as_ptr())
                                   .collect::<Vec<_>>();

        let create_info = vk::InstanceCreateInfo {
            sType: vk::STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            pApplicationInfo: &app_info,
            enabledLayerCount: layers.len() as u32,
            ppEnabledLayerNames: str_pointers.as_ptr(),
            enabledExtensionCount: (extensions.len() + surface_extensions.len()) as u32,
            ppEnabledExtensionNames: str_pointers[layers.len()..].as_ptr(),
        };
        let mut out = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            entry_points.CreateInstance(&create_info, ptr::null(), &mut out)
        });
        out
    };

    let inst_pointers = vk::InstancePointers::load(|name| unsafe {
        mem::transmute(lib.GetInstanceProcAddr(instance, name.as_ptr()))
    });

    let physical_devices = {
        let mut num = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            inst_pointers.EnumeratePhysicalDevices(instance, &mut num, ptr::null_mut())
        });
        let mut devices = Vec::with_capacity(num as usize);
        assert_eq!(vk::SUCCESS, unsafe {
            inst_pointers.EnumeratePhysicalDevices(instance, &mut num, devices.as_mut_ptr())
        });
        unsafe { devices.set_len(num as usize); }
        devices
    };
    
    let devices = physical_devices.iter()
        .map(|dev| PhysicalDeviceInfo::new(*dev, &inst_pointers))
        .collect::<Vec<_>>();

    let (dev, (qf_id, _))  = devices.iter()
        .flat_map(|d| iter::repeat(d).zip(d.queue_families.iter().enumerate()))
        .find(|&(_, (_, qf))| qf.queueFlags & vk::QUEUE_GRAPHICS_BIT != 0)
        .unwrap();
    info!("Chosen physical device {:?} with queue family {}", dev.device, qf_id);

    let mvid_id = dev.memory.memoryTypes.iter().take(dev.memory.memoryTypeCount as usize)
                            .position(|mt| (mt.propertyFlags & vk::MEMORY_PROPERTY_DEVICE_LOCAL_BIT != 0))
                            .unwrap() as u32;
    let msys_id = dev.memory.memoryTypes.iter().take(dev.memory.memoryTypeCount as usize)
                            .position(|mt| (mt.propertyFlags & vk::MEMORY_PROPERTY_HOST_COHERENT_BIT != 0)
                                        && (mt.propertyFlags & vk::MEMORY_PROPERTY_HOST_VISIBLE_BIT != 0))
                            .unwrap() as u32;

    let device = {
        let cstrings = dev_extensions.iter()
                                     .map(|&s| CString::new(s).unwrap())
                                     .collect::<Vec<_>>();
        let str_pointers = cstrings.iter().map(|s| s.as_ptr())
                                   .collect::<Vec<_>>();

        let queue_info = vk::DeviceQueueCreateInfo {
            sType: vk::STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
            pNext: ptr::null(),
            flags: 0,
            queueFamilyIndex: qf_id as u32,
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
            enabledExtensionCount: str_pointers.len() as u32,
            ppEnabledExtensionNames: str_pointers.as_ptr(),
            pEnabledFeatures: &features,
        };
        let mut out = 0;
        assert_eq!(vk::SUCCESS, unsafe {
            inst_pointers.CreateDevice(dev.device, &dev_info, ptr::null(), &mut out)
        });
        out
    };

    let dev_pointers = vk::DevicePointers::load(|name| unsafe {
        inst_pointers.GetDeviceProcAddr(device, name.as_ptr()) as *const _
    });
    let queue = unsafe {
        let mut out = mem::zeroed();
        dev_pointers.GetDeviceQueue(device, qf_id as u32, 0, &mut out);
        out
    };

    let share = Arc::new(Share {
        _dynamic_lib: dynamic_lib,
        _library: lib,
        instance: instance,
        inst_pointers: inst_pointers,
        device: device,
        dev_pointers: dev_pointers,
        physical_device: dev.device,
        handles: Mutex::new(core::handle::Manager::new()),
    });
    let gfx_device = command::GraphicsQueue::new(share.clone(), queue, qf_id as u32);
    let gfx_factory = factory::Factory::new(share.clone(), qf_id as u32, mvid_id, msys_id);

    (gfx_device, gfx_factory, share)
}


#[derive(Clone, PartialEq, Eq)]
pub struct Error(pub vk::Result);

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


#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources {}

impl core::Resources for Resources {
    type Buffer               = native::Buffer;
    type Shader               = native::Shader;
    type Program              = native::Program;
    type PipelineStateObject  = native::Pipeline;
    type Texture              = native::Texture;
    type ShaderResourceView   = native::TextureView; //TODO: buffer view
    type UnorderedAccessView  = ();
    type RenderTargetView     = native::TextureView;
    type DepthStencilView     = native::TextureView;
    type Sampler              = vk::Sampler;
    type Fence                = Fence;
    type Mapping              = factory::MappingGate;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Fence(vk::Fence);
