#[macro_use]
extern crate log;

extern crate ash;
extern crate gfx_core as core;
#[macro_use]
extern crate lazy_static;
extern crate smallvec;

#[cfg(target_os = "windows")]
extern crate kernel32;

use ash::{Entry, LoadingError};
use ash::version::{EntryV1_0, DeviceV1_0, InstanceV1_0, V1_0};
use ash::vk;
use core::{command as com, memory};
use core::QueueType;
use std::{fmt, mem, ptr};
use std::ffi::{CStr, CString};
use std::sync::Arc;

mod command;
mod conversions;
mod device;
pub mod native;
mod pool;


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
}

pub struct Instance {
    pub raw: ash::Instance<ash::version::V1_0>,

    /// Supported surface extensions of this instance.
    pub surface_extensions: Vec<&'static str>,
}

fn map_queue_type(flags: vk::QueueFlags) -> QueueType {
    if flags.subset(vk::QUEUE_GRAPHICS_BIT | vk::QUEUE_COMPUTE_BIT) { // TRANSER_BIT optional
        QueueType::General
    } else if flags.subset(vk::QUEUE_GRAPHICS_BIT) { // TRANSER_BIT optional
        QueueType::Graphics
    } else if flags.subset(vk::QUEUE_COMPUTE_BIT) { // TRANSER_BIT optional
        QueueType::Compute
    } else if flags.subset(vk::QUEUE_TRANSFER_BIT) {
        QueueType::Transfer
    } else {
        // TODO: present only queues?
        unimplemented!()
    }
}

impl Instance {
    pub fn create() -> Instance {
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

    pub fn enumerate_adapters(instance: Arc<Instance>) -> Vec<Adapter> {
        instance.raw.enumerate_physical_devices()
            .expect("Unable to enumerate adapter")
            .iter()
            .map(|&device| {
                let properties = instance.raw.get_physical_device_properties(device);
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

                let queue_families =
                    instance.raw.get_physical_device_queue_family_properties(device)
                        .iter()
                        .enumerate()
                        .map(|(i, queue_family)| {
                        (
                            QueueFamily {
                                device: device,
                                family_index: i as u32,
                                queue_type: queue_family.queue_flags,
                                queue_count: queue_family.queue_count,
                            },
                            map_queue_type(queue_family.queue_flags),
                        )
                        }).collect();

                Adapter {
                    instance: instance.clone(),
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
    instance: Arc<Instance>,
    handle: vk::PhysicalDevice,
    queue_families: Vec<(QueueFamily, QueueType)>,
    info: core::AdapterInfo,
}

impl core::Adapter<Backend> for Adapter {
    fn open(&self, queue_descs: &[(&QueueFamily, QueueType, u32)]) -> core::Gpu<Backend>
    {
        let mut queue_priorities = Vec::with_capacity(queue_descs.len());

        let queue_infos = queue_descs.iter().map(|&(family, _, queue_count)| {
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
                self.instance.raw.create_device(self.handle, &info, None)
                    .expect("Error on device creation")
            }
        };

        let device = Device {
            raw: Arc::new(RawDevice(device_raw)),
        };

        let mem_properties =  self.instance.raw.get_physical_device_memory_properties(self.handle);
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
                    device.raw.0.get_device_queue(info.queue_family_index, id)
                };
                unimplemented!()
                /*
                // TODO:
                unsafe {
                    core::GeneralQueue::new(CommandQueue {
                        inner: CommandQueueInner(Rc::new(RefCell::new(queue))),
                        device: device.device.clone(),
                        family_index: info.queue_family_index,
                    })
                }
                */
            }).collect::<Vec<_>>()
        }).collect();

        core::Gpu {
            device,
            general_queues: queues,
            graphics_queues: Vec::new(),
            compute_queues: Vec::new(),
            transfer_queues: Vec::new(),
            heap_types,
            memory_heaps,
        }
    }

    fn get_info(&self) -> &core::AdapterInfo {
        &self.info
    }

    fn get_queue_families(&self) -> &[(QueueFamily, QueueType)] {
        &self.queue_families
    }
}

#[doc(hidden)]
pub struct RawDevice(pub ash::Device<V1_0>);
impl fmt::Debug for RawDevice {
    fn fmt(&self, _formatter: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!()
    }
}
impl Drop for RawDevice {
    fn drop(&mut self) {
        unsafe { self.0.destroy_device(None); }
    }
}

// Need to explicitly synchronize on submission and present.
pub type RawCommandQueue = Arc<vk::Queue>;

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
    unsafe fn submit_raw<'a, I>(
        &mut self,
        submit_infos: I,
        fence: Option<&native::Fence>,
    ) where I: Iterator<Item=core::RawSubmission<'a, Backend>> {
        unimplemented!()
    }
}

pub struct Device {
    raw: Arc<RawDevice>,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl core::Backend for Backend {
    type Adapter = Adapter;
    type Device = Device;

    type CommandQueue = CommandQueue;
    type RawCommandBuffer = command::CommandBuffer;
    type SubpassCommandBuffer = command::SubpassCommandBuffer;
    type SubmitInfo = command::SubmitInfo;
    type QueueFamily = QueueFamily;

    type Heap = native::Heap;
    type Mapping = device::Mapping;
    type RawCommandPool = pool::RawCommandPool;
    type SubpassCommandPool = pool::SubpassCommandPool;

    type ShaderLib = native::ShaderLib;
    type RenderPass = native::RenderPass;
    type FrameBuffer = native::FrameBuffer;

    type UnboundBuffer = device::UnboundBuffer;
    type Buffer = native::Buffer;
    type UnboundImage = device::UnboundImage;
    type Image = native::Image;
    type Sampler = native::Sampler;

    type ConstantBufferView = native::ConstantBufferView;
    type ShaderResourceView = native::ShaderResourceView;
    type UnorderedAccessView = native::UnorderedAccessView;
    type RenderTargetView = native::RenderTargetView;
    type DepthStencilView = native::DepthStencilView;

    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineLayout = native::PipelineLayout;
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type DescriptorPool = native::DescriptorPool;
    type DescriptorSet = native::DescriptorSet;

    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
}
