#[macro_use]
extern crate log;
extern crate ash;
extern crate byteorder;
extern crate gfx_hal as hal;
#[macro_use]
extern crate lazy_static;
extern crate smallvec;
#[cfg(windows)]
extern crate kernel32;
#[cfg(windows)]
extern crate user32;
#[cfg(windows)]
extern crate winapi;
#[cfg(feature = "winit")]
extern crate winit;
#[cfg(all(unix, not(target_os = "android")))]
extern crate x11;
#[cfg(feature = "glsl-to-spirv")]
extern crate glsl_to_spirv;

use ash::{Entry, LoadingError};
use ash::extensions as ext;
use ash::version::{EntryV1_0, DeviceV1_0, InstanceV1_0, V1_0};
use ash::vk;

use hal::{format, memory};
use hal::{Features, Limits, PatchSize, QueueType};
use hal::adapter::DeviceCreationError;

use std::{fmt, mem, ptr};
use std::ffi::{CStr, CString};
use std::sync::Arc;

mod command;
mod conv;
mod device;
mod native;
mod pool;
mod result;
mod window;

const LAYERS: &'static [&'static str] = &[
    #[cfg(debug_assertions)]
    "VK_LAYER_LUNARG_standard_validation",
];
const EXTENSIONS: &'static [&'static str] = &[
    #[cfg(debug_assertions)]
    "VK_EXT_debug_report",
];
const DEVICE_EXTENSIONS: &'static [&'static str] = &[
    vk::VK_KHR_SWAPCHAIN_EXTENSION_NAME,
];
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

pub struct RawInstance(pub ash::Instance<V1_0>);
impl Drop for RawInstance {
    fn drop(&mut self) {
        unsafe { self.0.destroy_instance(None); }
    }
}

pub struct Instance {
    pub raw: Arc<RawInstance>,

    /// Supported extensions of this instance.
    pub extensions: Vec<&'static str>,

    // TODO: move into `RawInstance`, destroy in `drop`
    _debug_report: Option<(ext::DebugReport, vk::DebugReportCallbackEXT)>,
}

fn map_queue_type(flags: vk::QueueFlags) -> QueueType {
    if flags.subset(vk::QUEUE_GRAPHICS_BIT | vk::QUEUE_COMPUTE_BIT) { // TRANSFER_BIT optional
        QueueType::General
    } else if flags.subset(vk::QUEUE_GRAPHICS_BIT) { // TRANSFER_BIT optional
        QueueType::Graphics
    } else if flags.subset(vk::QUEUE_COMPUTE_BIT) { // TRANSFER_BIT optional
        QueueType::Compute
    } else if flags.subset(vk::QUEUE_TRANSFER_BIT) {
        QueueType::Transfer
    } else {
        // TODO: present only queues?
        unimplemented!()
    }
}

extern "system" fn callback(
    type_: vk::DebugReportFlagsEXT,
    _: vk::DebugReportObjectTypeEXT,
    _object: u64,
    _location: usize,
    _msg_code: i32,
    layer_prefix: *const vk::types::c_char,
    description: *const vk::types::c_char,
    _user_data: *mut vk::types::c_void,
) -> vk::Bool32 {
    unsafe {
        let level = match type_ {
            vk::DEBUG_REPORT_ERROR_BIT_EXT => log::LogLevel::Error,
            vk::DEBUG_REPORT_DEBUG_BIT_EXT => log::LogLevel::Debug,
            _ => log::LogLevel::Warn,
        };
        let layer_prefix = CStr::from_ptr(layer_prefix).to_str().unwrap();
        let description = CStr::from_ptr(description).to_str().unwrap();
        log!(level, "[{}] {}", layer_prefix, description);
        vk::VK_FALSE
    }
}

impl Instance {
    pub fn create(name: &str, version: u32) -> Self {
        // TODO: return errors instead of panic
        let entry = VK_ENTRY.as_ref().expect("Unable to load Vulkan entry points");

        let app_name = CString::new(name).unwrap();
        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::ApplicationInfo,
            p_next: ptr::null(),
            p_application_name: app_name.as_ptr(),
            application_version: version,
            p_engine_name: b"gfx-rs\0".as_ptr() as *const _,
            engine_version: 1,
            api_version: 0, //TODO
        };

        let instance_extensions = entry
            .enumerate_instance_extension_properties()
            .expect("Unable to enumerate instance extensions");

        let instance_layers = entry
            .enumerate_instance_layer_properties()
            .expect("Unable to enumerate instance layers");

        // Check our xtensions against the available extensions
        let extensions = SURFACE_EXTENSIONS
            .iter()
            .chain(EXTENSIONS.iter())
            .filter_map(|&ext| {
                instance_extensions
                    .iter()
                    .find(|inst_ext| unsafe {
                        CStr::from_ptr(inst_ext.extension_name.as_ptr()) ==
                            CStr::from_ptr(ext.as_ptr() as *const _)
                    })
                    .map(|_| ext)
                    .or_else(|| {
                        warn!("Unable to find extension: {}", ext);
                        None
                    })
            })
            .collect::<Vec<&str>>();

        // Check requested layers against the available layers
        let layers = LAYERS
            .iter()
            .filter_map(|&layer| {
                instance_layers
                    .iter()
                    .find(|inst_layer| unsafe {
                        CStr::from_ptr(inst_layer.layer_name.as_ptr()) ==
                            CStr::from_ptr(layer.as_ptr() as *const _)
                    })
                    .map(|_| layer)
                    .or_else(|| {
                        warn!("Unable to find layer: {}", layer);
                        None
                    })
            })
            .collect::<Vec<&str>>();

        let instance = {
            let cstrings = layers
                .iter()
                .chain(extensions.iter())
                .map(|&s| CString::new(s).unwrap())
                .collect::<Vec<_>>();

            let str_pointers = cstrings
                .iter()
                .map(|s| s.as_ptr())
                .collect::<Vec<_>>();

            let create_info = vk::InstanceCreateInfo {
                s_type: vk::StructureType::InstanceCreateInfo,
                p_next: ptr::null(),
                flags: vk::InstanceCreateFlags::empty(),
                p_application_info: &app_info,
                enabled_layer_count: layers.len() as _,
                pp_enabled_layer_names: str_pointers.as_ptr(),
                enabled_extension_count: extensions.len() as _,
                pp_enabled_extension_names: str_pointers[layers.len()..].as_ptr(),
            };

            unsafe {
                entry.create_instance(&create_info, None)
            }.expect("Unable to create Vulkan instance")
        };

        #[cfg(debug_assertions)]
        let debug_report = {
            let ext = ext::DebugReport::new(entry, &instance).unwrap();
            let info = vk::DebugReportCallbackCreateInfoEXT {
                s_type: vk::StructureType::DebugReportCallbackCreateInfoExt,
                p_next: ptr::null(),
                flags: vk::DEBUG_REPORT_WARNING_BIT_EXT |
                       vk::DEBUG_REPORT_PERFORMANCE_WARNING_BIT_EXT |
                       vk::DEBUG_REPORT_ERROR_BIT_EXT,
                pfn_callback: callback,
                p_user_data: ptr::null_mut(),
            };
            let handle = unsafe {
                ext.create_debug_report_callback_ext(&info, None)
            }.unwrap();
            Some((ext, handle))
        };
        #[cfg(not(debug_assertions))]
        let debug_report = None;

        Instance {
            raw: Arc::new(RawInstance(instance)),
            extensions,
            _debug_report: debug_report,
        }
    }
}

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        self.raw.0.enumerate_physical_devices()
            .expect("Unable to enumerate adapter")
            .into_iter()
            .map(|device| {
                let properties = self.raw.0.get_physical_device_properties(device);
                let info = hal::AdapterInfo {
                    name: unsafe {
                        CStr::from_ptr(properties.device_name.as_ptr())
                            .to_str()
                            .expect("Invalid UTF-8 string")
                            .to_owned()
                    },
                    vendor: properties.vendor_id as usize,
                    device: properties.device_id as usize,
                    software_rendering: properties.device_type == vk::PhysicalDeviceType::Cpu,
                };
                let physical_device = PhysicalDevice {
                    instance: self.raw.clone(),
                    handle: device,
                    properties,
                };
                let queue_families = self.raw.0
                    .get_physical_device_queue_family_properties(device)
                    .into_iter()
                    .enumerate()
                    .map(|(i, properties)| {
                        QueueFamily {
                            properties,
                            device,
                            index: i as u32,
                        }
                    }).collect();
                hal::Adapter {
                    info,
                    physical_device,
                    queue_families,
                }
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct QueueFamily {
    properties: vk::QueueFamilyProperties,
    device: vk::PhysicalDevice,
    index: u32,
}

impl hal::queue::QueueFamily for QueueFamily {
    fn queue_type(&self) -> QueueType {
        map_queue_type(self.properties.queue_flags)
    }
    fn max_queues(&self) -> usize {
        self.properties.queue_count as _
    }
}


pub struct PhysicalDevice {
    instance: Arc<RawInstance>,
    handle: vk::PhysicalDevice,
    properties: vk::PhysicalDeviceProperties,
}

impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(
        &self, families: Vec<(QueueFamily, Vec<hal::QueuePriority>)>
    ) -> Result<hal::Gpu<Backend>, DeviceCreationError> {
        let family_infos = families
            .iter()
            .map(|&(ref family, ref priorities)| vk::DeviceQueueCreateInfo {
                s_type: vk::StructureType::DeviceQueueCreateInfo,
                p_next: ptr::null(),
                flags: vk::DeviceQueueCreateFlags::empty(),
                queue_family_index: family.index,
                queue_count: priorities.len() as _,
                p_queue_priorities: priorities.as_ptr(),
            })
            .collect::<Vec<_>>();

        // Create device
        let device_raw = {
            let cstrings = DEVICE_EXTENSIONS
                .iter()
                .map(|&s| CString::new(s).unwrap())
                .collect::<Vec<_>>();

            let str_pointers = cstrings
                .iter()
                .map(|s| s.as_ptr())
                .collect::<Vec<_>>();

            let features = unsafe { mem::zeroed() };
            let info = vk::DeviceCreateInfo {
                s_type: vk::StructureType::DeviceCreateInfo,
                p_next: ptr::null(),
                flags: vk::DeviceCreateFlags::empty(),
                queue_create_info_count: family_infos.len() as u32,
                p_queue_create_infos: family_infos.as_ptr(),
                enabled_layer_count: 0,
                pp_enabled_layer_names: ptr::null(),
                enabled_extension_count: str_pointers.len() as u32,
                pp_enabled_extension_names: str_pointers.as_ptr(),
                p_enabled_features: &features,
            };

            unsafe {
                self.instance
                    .0
                    .create_device(self.handle, &info, None)
                    .map_err(|err| {
                        match err {
                            ash::DeviceError::LoadError(err) => panic!("{:?}", err),
                            ash::DeviceError::VkError(err) => Into::<result::Error>::into(err),
                        }
                    })
                    .map_err(Into::<DeviceCreationError>::into)?
            }
        };

        let device = Device {
            raw: Arc::new(RawDevice(device_raw)),
        };

        let device_arc = device.raw.clone();
        let queue_groups = families
            .into_iter()
            .map(|(family, priorities)| {
                let family_index = family.index;
                let mut family_raw = hal::queue::RawQueueGroup::new(family);
                for id in 0 .. priorities.len() {
                    let queue_raw = unsafe {
                        device_arc.0.get_device_queue(family_index, id as _)
                    };
                    family_raw.add_queue(CommandQueue {
                        raw: Arc::new(queue_raw),
                        device: device_arc.clone(),
                    });
                }
                family_raw
            })
            .collect();

        Ok(hal::Gpu {
            device,
            queue_groups,
        })
    }

    fn format_properties(&self, format: Option<format::Format>) -> format::Properties {
        let properties = self
            .instance
            .0
            .get_physical_device_format_properties(
                self.handle,
                format.map_or(vk::Format::Undefined, conv::map_format),
            );

        format::Properties {
            linear_tiling: conv::map_image_features(properties.linear_tiling_features),
            optimal_tiling: conv::map_image_features(properties.optimal_tiling_features),
            buffer_features: conv::map_buffer_features(properties.buffer_features),
        }
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        let mem_properties = self.instance.0.get_physical_device_memory_properties(self.handle);
        let memory_heaps = mem_properties.memory_heaps[..mem_properties.memory_heap_count as usize]
            .iter()
            .map(|mem| mem.size).collect();
        let memory_types = mem_properties
            .memory_types[..mem_properties.memory_type_count as usize]
            .iter()
            .map(|mem| {
                use memory::Properties;
                let mut type_flags = Properties::empty();

                if mem.property_flags.intersects(vk::MEMORY_PROPERTY_DEVICE_LOCAL_BIT) {
                    type_flags |= Properties::DEVICE_LOCAL;
                }
                if mem.property_flags.intersects(vk::MEMORY_PROPERTY_HOST_COHERENT_BIT) {
                    type_flags |= Properties::COHERENT;
                }
                if mem.property_flags.intersects(vk::MEMORY_PROPERTY_HOST_CACHED_BIT) {
                    type_flags |= Properties::CPU_CACHED;
                }
                if mem.property_flags.intersects(vk::MEMORY_PROPERTY_HOST_VISIBLE_BIT) {
                    type_flags |= Properties::CPU_VISIBLE;
                }
                if mem.property_flags.intersects(vk::MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT) {
                    type_flags |= Properties::LAZILY_ALLOCATED;
                }

                hal::MemoryType {
                    properties: type_flags,
                    heap_index: mem.heap_index as usize,
                }
            })
            .collect();

        hal::MemoryProperties {
            memory_heaps,
            memory_types,
        }
    }

    fn get_features(&self) -> Features {
        let limits = &self.properties.limits;

        Features { //TODO
            indirect_execution: limits.max_draw_indirect_count != 0,
            draw_instanced: false,
            draw_instanced_base: false,
            draw_indexed_base: false,
            draw_indexed_instanced: false,
            draw_indexed_instanced_base_vertex: false,
            draw_indexed_instanced_base: false,
            instance_rate: false,
            vertex_base: false,
            srgb_color: false,
            constant_buffer: false,
            unordered_access_view: false,
            separate_blending_slots: false,
            copy_buffer: false,
            sampler_anisotropy: false,
            sampler_border_color: false,
            sampler_lod_bias: false,
            sampler_objects: false,
            precise_occlusion_query: false,
            pipeline_statistics_query: false,
        }
    }
    fn get_limits(&self) -> Limits {
        let limits = &self.properties.limits;
        let max_group_count = limits.max_compute_work_group_count;
        let max_group_size = limits.max_compute_work_group_size;

        Limits {
            max_texture_size: limits.max_image_dimension3d as _,
            max_patch_size: limits.max_tessellation_patch_size as PatchSize,
            max_viewports: limits.max_viewports as _,
            max_compute_group_count: [max_group_count[0] as _, max_group_count[1] as _, max_group_count[2] as _],
            max_compute_group_size: [max_group_size[0] as _, max_group_size[1] as _, max_group_size[2] as _],
            min_buffer_copy_offset_alignment: limits.optimal_buffer_copy_offset_alignment as _,
            min_buffer_copy_pitch_alignment: limits.optimal_buffer_copy_row_pitch_alignment as _,
            min_uniform_buffer_offset_alignment: limits.min_uniform_buffer_offset_alignment as _,
        }
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
}

impl hal::queue::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit_raw(&mut self,
        submission: hal::queue::RawSubmission<Backend>,
        fence: Option<&native::Fence>,
    ) {
        let buffers = submission.cmd_buffers
            .iter()
            .map(|cmd| cmd.raw)
            .collect::<Vec<_>>();
        let waits = submission.wait_semaphores
            .iter()
            .map(|&(ref semaphore, _)| semaphore.0)
            .collect::<Vec<_>>();
        let stages = submission.wait_semaphores
            .iter()
            .map(|&(_, stage)| conv::map_pipeline_stage(stage))
            .collect::<Vec<_>>();
        let signals = submission.signal_semaphores
            .iter()
            .map(|semaphore| semaphore.0)
            .collect::<Vec<_>>();

        let info = vk::SubmitInfo {
            s_type: vk::StructureType::SubmitInfo,
            p_next: ptr::null(),
            wait_semaphore_count: waits.len() as u32,
            p_wait_semaphores: waits.as_ptr(),
            // If count is zero, AMD driver crashes if nullptr is not set for stage masks
            p_wait_dst_stage_mask: if stages.is_empty() { ptr::null() } else { stages.as_ptr() },
            command_buffer_count: buffers.len() as u32,
            p_command_buffers: buffers.as_ptr(),
            signal_semaphore_count: signals.len() as u32,
            p_signal_semaphores: signals.as_ptr(),
        };

        let fence_raw = fence
            .map(|fence| fence.0)
            .unwrap_or(vk::Fence::null());

        let result = self.device.0.queue_submit(*self.raw, &[info], fence_raw);
        assert_eq!(Ok(()), result);
    }
}

pub struct Device {
    raw: Arc<RawDevice>,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Backend {}
impl hal::Backend for Backend {
    type PhysicalDevice = PhysicalDevice;
    type Device = Device;

    type Surface = window::Surface;
    type Swapchain = window::Swapchain;

    type QueueFamily = QueueFamily;
    type CommandQueue = CommandQueue;
    type CommandBuffer = command::CommandBuffer;
    type SubpassCommandBuffer = command::SubpassCommandBuffer;

    type Memory = native::Memory;
    type CommandPool = pool::RawCommandPool;
    type SubpassCommandPool = pool::SubpassCommandPool;

    type ShaderModule = native::ShaderModule;
    type RenderPass = native::RenderPass;
    type Framebuffer = native::FrameBuffer;

    type UnboundBuffer = device::UnboundBuffer;
    type Buffer = native::Buffer;
    type BufferView = native::BufferView;
    type UnboundImage = device::UnboundImage;
    type Image = native::Image;
    type ImageView = native::ImageView;
    type Sampler = native::Sampler;

    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineLayout = native::PipelineLayout;
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type DescriptorPool = native::DescriptorPool;
    type DescriptorSet = native::DescriptorSet;

    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
    type QueryPool = native::QueryPool;
}
