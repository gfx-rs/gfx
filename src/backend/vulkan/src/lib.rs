// Copyright 2017 The Gfx-rs Developers.
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

extern crate ash;
extern crate gfx_core as core;
#[macro_use]
extern crate lazy_static;

#[cfg(target_os = "windows")]
extern crate kernel32;

use ash::{Entry, LoadingError};
use ash::version::{EntryV1_0, DeviceV1_0, InstanceV1_0, V1_0};
use ash::vk;
use core::memory;
use core::{CommandBuffer, FrameSync};
use std::{mem, ptr};
use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

mod command;
pub mod data;
mod factory;
pub mod native;
mod pool;

pub use pool::{GeneralCommandPool, GraphicsCommandPool,
    ComputeCommandPool, TransferCommandPool, SubpassCommandPool};

const SURFACE_EXTENSIONS: &'static [&'static str] = &[
    vk::VK_KHR_SURFACE_EXTENSION_NAME,

    // Platform-specific WSI extensions
    vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_XCB_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_MIR_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_ANDROID_SURFACE_EXTENSION_NAME,
    vk::VK_KHR_WIN32_SURFACE_EXTENSION_NAME,
];

lazy_static! {
    // Entry function pointers
    pub static ref VK_ENTRY: Result<Entry<V1_0>, LoadingError> = Entry::new();

    pub static ref INSTANCE: Instance = Instance::create();
}

pub struct Instance {
    pub raw: ash::Instance<ash::version::V1_0>,

    /// Supported surface extensions of this instance.
    surface_extensions: Vec<&'static str>,
}

impl Instance {
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
            raw: instance,
            surface_extensions: surface_extensions,
        }
    }

    fn enumerate_adapters(&self) -> Vec<Adapter> {
        self.raw.enumerate_physical_devices()
            .expect("Unable to enumerate adapter")
            .iter()
            .map(|&device| {
                let properties = self.raw.get_physical_device_properties(device);
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

                let queue_families = self.raw.get_physical_device_queue_family_properties(device)
                                                 .iter()
                                                 .enumerate()
                                                 .map(|(i, queue_family)| {
                                                    QueueFamily {
                                                        device: device,
                                                        family_index: i as u32,
                                                        queue_type: queue_family.queue_flags,
                                                        queue_count: queue_family.queue_count,
                                                    }
                                                 }).collect();

                Adapter {
                    handle: device,
                    queue_families: queue_families,
                    info: info,
                }
            })
            .collect()
    }
}

pub struct QueueFamily {
    device: vk::PhysicalDevice,
    family_index: u32,
    queue_type: vk::QueueFlags,
    queue_count: u32,
}

impl QueueFamily {
    #[doc(hidden)]
    pub fn from_raw(device: vk::PhysicalDevice, index: u32, properties: &vk::QueueFamilyProperties) -> Self {
        QueueFamily {
            device: device,
            family_index: index,
            queue_type: properties.queue_flags,
            queue_count: properties.queue_count,
        }
    }

    #[doc(hidden)]
    pub fn device(&self) -> vk::PhysicalDevice {
        self.device
    }

    #[doc(hidden)]
    pub fn family_index(&self) -> u32 {
        self.family_index
    }

}

impl core::QueueFamily for QueueFamily {
    fn num_queues(&self) -> u32 {
        self.queue_count
    }
}

pub struct Adapter {
    handle: vk::PhysicalDevice,
    queue_families: Vec<QueueFamily>,
    info: core::AdapterInfo,
}

impl Adapter {
    #[doc(hidden)]
    pub fn from_raw(device: vk::PhysicalDevice, queue_families: Vec<QueueFamily>, info: core::AdapterInfo) -> Self {
        Adapter {
            handle: device,
            queue_families: queue_families,
            info: info,
        }
    }
}

impl core::Adapter<Backend> for Adapter {
    fn open(&self, queue_descs: &[(&QueueFamily, u32)]) -> core::Device_<Backend>
    {
        let mut queue_priorities = Vec::with_capacity(queue_descs.len());

        let queue_infos = queue_descs.iter().map(|&(family, queue_count)| {
                queue_priorities.push(vec![0.0f32; queue_count as usize]);

                vk::DeviceQueueCreateInfo {
                    s_type: vk::StructureType::DeviceQueueCreateInfo,
                    p_next: ptr::null(),
                    flags: vk::DeviceQueueCreateFlags::empty(),
                    queue_family_index: family.family_index,
                    queue_count: queue_count,
                    p_queue_priorities: queue_priorities.last().unwrap().as_ptr(),
                }
            }).collect::<Vec<_>>();

        // Create device
        let device_extensions = &[vk::VK_KHR_SWAPCHAIN_EXTENSION_NAME,];

        let device_raw = {
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
                INSTANCE.raw.create_device(self.handle, &info, None)
                    .expect("Error on device creation")
            }
        };

        let factory = Factory {
            device: Arc::new(RawDevice(device_raw)),
        };

        let mem_properties =  INSTANCE.raw.get_physical_device_memory_properties(self.handle);
        let memory_heaps = mem_properties.memory_heaps[..mem_properties.memory_heap_count as usize].iter()
                                .map(|mem| mem.size).collect::<Vec<_>>();
        let heap_types = mem_properties.memory_types[..mem_properties.memory_type_count as usize].iter().enumerate().map(|(i, mem)| {
            let mut type_flags = memory::HeapProperties::empty();

            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_DEVICE_LOCAL_BIT) {
                type_flags |= memory::DEVICE_LOCAL;
            }
            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_HOST_COHERENT_BIT) {
                type_flags |= memory::COHERENT;
            }
            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_HOST_CACHED_BIT) {
                type_flags |= memory::CPU_CACHED;
            }
            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_HOST_VISIBLE_BIT) {
                type_flags |= memory::CPU_VISIBLE;
            }
            if mem.property_flags.intersects(vk::MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT) {
                type_flags |= memory::LAZILY_ALLOCATED;
            }
            
            core::HeapType {
                id: i,
                properties: type_flags,
                heap_index: mem.heap_index as usize,
            }
        }).collect::<Vec<_>>();

        // Create associated command queues for each queue type
        let queues = queue_infos.iter().flat_map(|info| {
            (0..info.queue_count).map(|id| {
                let queue = unsafe {
                    factory.device.0.get_device_queue(info.queue_family_index, id)
                };
                unimplemented!()
                /*
                // TODO:
                unsafe {
                    core::GeneralQueue::new(CommandQueue {
                        inner: CommandQueueInner(Rc::new(RefCell::new(queue))),
                        device: factory.device.clone(),
                        family_index: info.queue_family_index,
                    })
                }
                */
            }).collect::<Vec<_>>()
        }).collect();

        core::Device_ {
            factory: factory,
            general_queues: queues,
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            heap_types: heap_types,
            memory_heaps: memory_heaps,

            _marker: std::marker::PhantomData,
        }
    }

    fn get_info(&self) -> &core::AdapterInfo {
        &self.info
    }

    fn get_queue_families(&self) -> &[QueueFamily] {
        &self.queue_families
    }
}

#[doc(hidden)]
pub struct RawDevice(pub ash::Device<V1_0>);
impl Drop for RawDevice {
    fn drop(&mut self) {
        unsafe { self.0.destroy_device(None); }
    }
}

// Need to explicitly synchronize on submission and present.
// TODO: Can we avoid somehow the use of a mutex?
pub type RawCommandQueue = Arc<Mutex<vk::Queue>>;

pub struct CommandQueue {
    raw: RawCommandQueue,
    device: Arc<RawDevice>,
    family_index: u32,
}

impl CommandQueue {
    #[doc(hidden)]
    pub fn raw(&self) -> RawCommandQueue {
        self.raw.clone()
    }

    #[doc(hidden)]
    pub fn device(&self) -> Arc<RawDevice> {
        self.device.clone()
    }

    #[doc(hidden)]
    pub fn device_handle(&self) -> vk::Device {
        self.device.0.handle()
    }
}

impl core::CommandQueue<Backend> for CommandQueue {
    unsafe fn submit(&mut self, submit_infos: &[core::QueueSubmit<Backend>], fence: Option<&mut native::Fence>) {

        unimplemented!()
    }

    fn wait_idle(&mut self) {
        unsafe {
            self.device.0.queue_wait_idle(*self.raw.lock().unwrap());
        }
    }
}

pub struct Factory {
    device: Arc<RawDevice>,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl core::Backend for Backend {
    type Resources = Resources;
    type CommandQueue = CommandQueue;
    type GeneralCommandBuffer = native::GeneralCommandBuffer;
    type GraphicsCommandBuffer = native::GraphicsCommandBuffer;
    type ComputeCommandBuffer = native::ComputeCommandBuffer;
    type TransferCommandBuffer = native::TransferCommandBuffer;
    type SubpassCommandBuffer = native::SubpassCommandBuffer;
    type SubmitInfo = command::SubmitInfo;
    type Factory = Factory;
    type QueueFamily = QueueFamily;
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Resources { }
impl core::Resources for Resources {
    type Buffer = ();
    type Shader = ();
    type Program = ();
    type PipelineStateObject = ();
    type Texture = ();
    type ShaderResourceView = ();
    type UnorderedAccessView = ();
    type RenderTargetView = ();
    type DepthStencilView = ();
    type Sampler = ();
    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
    type Mapping = Mapping;
}

// TODO: temporary
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct Mapping;

impl core::mapping::Gate<Resources> for Mapping {
    unsafe fn set<T>(&self, index: usize, val: T) {
        unimplemented!()
    }

    unsafe fn slice<'a, 'b, T>(&'a self, len: usize) -> &'b [T] {
        unimplemented!()
    }

    unsafe fn mut_slice<'a, 'b, T>(&'a self, len: usize) -> &'b mut [T] {
        unimplemented!()
    }
}

