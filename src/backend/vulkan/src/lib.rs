#![allow(non_snake_case)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate ash;
extern crate byteorder;
#[macro_use]
extern crate derivative;
extern crate gfx_hal as hal;
#[macro_use]
extern crate lazy_static;
#[cfg(feature = "use-rtld-next")]
extern crate shared_library;
extern crate smallvec;

#[cfg(windows)]
extern crate winapi;
#[cfg(feature = "winit")]
extern crate winit;
#[cfg(all(unix, not(target_os = "android")))]
extern crate x11;
#[cfg(all(unix, not(target_os = "android")))]
extern crate xcb;

use ash::extensions::{ext, khr};
use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::vk;
#[cfg(not(feature = "use-rtld-next"))]
use ash::{Entry, LoadingError};

use hal::adapter::DeviceType;
use hal::error::{DeviceCreationError, HostExecutionError};
use hal::device::{DeviceLost, OutOfMemory, SurfaceLost};
use hal::pso::PipelineStage;
use hal::{format, image, memory, queue, window::{PresentError, Suboptimal}};
use hal::{Features, Limits, PatchSize, QueueType, SwapImageIndex};

use std::borrow::{Borrow, Cow};
use std::ffi::{CStr, CString};
use std::sync::Arc;
use std::{fmt, mem, ptr, slice};

#[cfg(feature = "use-rtld-next")]
use ash::{EntryCustom, LoadingError};
#[cfg(feature = "use-rtld-next")]
use shared_library::dynamic_library::{DynamicLibrary, SpecialHandles};

mod command;
mod conv;
mod device;
mod info;
mod native;
mod pool;
mod result;
mod window;

// CStr's cannot be constant yet, until const fn lands we need to use a lazy_static
lazy_static! {
    static ref LAYERS: Vec<&'static CStr> = vec![#[cfg(debug_assertions)] CStr::from_bytes_with_nul(b"VK_LAYER_LUNARG_standard_validation\0").expect("Wrong extension string")];
    static ref EXTENSIONS: Vec<&'static CStr> = vec![#[cfg(debug_assertions)] CStr::from_bytes_with_nul(b"VK_EXT_debug_utils\0").expect("Wrong extension string")];
    static ref DEVICE_EXTENSIONS: Vec<&'static CStr> = vec![khr::Swapchain::name()];
    static ref SURFACE_EXTENSIONS: Vec<&'static CStr> = vec![
        khr::Surface::name(),
        // Platform-specific WSI extensions
        #[cfg(all(unix, not(target_os = "android")))]
        khr::XlibSurface::name(),
        #[cfg(all(unix, not(target_os = "android")))]
        khr::XcbSurface::name(),
        #[cfg(all(unix, not(target_os = "android")))]
        khr::WaylandSurface::name(),
        #[cfg(target_os = "android")]
        khr::AndroidSurface::name(),
        #[cfg(target_os = "windows")]
        khr::Win32Surface::name(),
    ];
}

#[cfg(not(feature = "use-rtld-next"))]
lazy_static! {
    // Entry function pointers
    pub static ref VK_ENTRY: Result<Entry, LoadingError> = Entry::new();
}

#[cfg(feature = "use-rtld-next")]
lazy_static! {
    // Entry function pointers
    pub static ref VK_ENTRY: Result<EntryCustom<V1_0, ()>, LoadingError>
        = EntryCustom::new_custom(
            || Ok(()),
            |_, name| unsafe {
                DynamicLibrary::symbol_special(SpecialHandles::Next, &*name.to_string_lossy())
                    .unwrap_or(ptr::null_mut())
            }
        );
}

pub struct RawInstance(
    pub ash::Instance,
    Option<(ext::DebugUtils, vk::DebugUtilsMessengerEXT)>,
);

impl Drop for RawInstance {
    fn drop(&mut self) {
        unsafe {
            #[cfg(debug_assertions)]
            {
                if let Some((ref ext, callback)) = self.1 {
                    ext.destroy_debug_utils_messenger(callback, None);
                }
            }

            self.0.destroy_instance(None);
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Instance {
    #[derivative(Debug = "ignore")]
    pub raw: Arc<RawInstance>,

    /// Supported extensions of this instance.
    pub extensions: Vec<&'static CStr>,
}

fn map_queue_type(flags: vk::QueueFlags) -> QueueType {
    if flags.contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE) {
        // TRANSFER_BIT optional
        QueueType::General
    } else if flags.contains(vk::QueueFlags::GRAPHICS) {
        // TRANSFER_BIT optional
        QueueType::Graphics
    } else if flags.contains(vk::QueueFlags::COMPUTE) {
        // TRANSFER_BIT optional
        QueueType::Compute
    } else if flags.contains(vk::QueueFlags::TRANSFER) {
        QueueType::Transfer
    } else {
        // TODO: present only queues?
        unimplemented!()
    }
}

unsafe fn display_debug_utils_label_ext(
    label_structs: *mut vk::DebugUtilsLabelEXT,
    count: usize,
) -> Option<String> {
    if count == 0 {
        return None;
    }

    Some(
        slice::from_raw_parts::<vk::DebugUtilsLabelEXT>(label_structs, count)
            .iter()
            .flat_map(|dul_obj| {
                dul_obj
                    .p_label_name
                    .as_ref()
                    .map(|lbl| CStr::from_ptr(lbl).to_string_lossy().into_owned())
            })
            .collect::<Vec<String>>()
            .join(", "),
    )
}

unsafe fn display_debug_utils_object_name_info_ext(
    info_structs: *mut vk::DebugUtilsObjectNameInfoEXT,
    count: usize,
) -> Option<String> {
    if count == 0 {
        return None;
    }

    //TODO: use color field of vk::DebugUtilsLabelsExt in a meaningful way?
    Some(
        slice::from_raw_parts::<vk::DebugUtilsObjectNameInfoEXT>(info_structs, count)
            .iter()
            .map(|obj_info| {
                let object_name = obj_info
                    .p_object_name
                    .as_ref()
                    .map(|name| CStr::from_ptr(name).to_string_lossy().into_owned());

                match object_name {
                    Some(name) => format!(
                        "(type: {:?}, hndl: {}, name: {})",
                        obj_info.object_type,
                        &obj_info.object_handle.to_string(),
                        name
                    ),
                    None => format!(
                        "(type: {:?}, hndl: {})",
                        obj_info.object_type,
                        &obj_info.object_handle.to_string()
                    ),
                }
            })
            .collect::<Vec<String>>()
            .join(", "),
    )
}

unsafe extern "system" fn debug_utils_messenger_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;

    let message_severity = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::Level::Error,
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => log::Level::Warn,
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => log::Level::Info,
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => log::Level::Trace,
        _ => log::Level::Warn,
    };
    let message_type = &format!("{:?}", message_type);
    let message_id_number: i32 = callback_data.message_id_number as i32;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    let additional_info: [(&str, Option<String>); 3] = [
        (
            "queue info",
            display_debug_utils_label_ext(
                callback_data.p_queue_labels as *mut _,
                callback_data.queue_label_count as usize,
            ),
        ),
        (
            "cmd buf info",
            display_debug_utils_label_ext(
                callback_data.p_cmd_buf_labels as *mut _,
                callback_data.cmd_buf_label_count as usize,
            ),
        ),
        (
            "object info",
            display_debug_utils_object_name_info_ext(
                callback_data.p_objects as *mut _,
                callback_data.object_count as usize,
            ),
        ),
    ];

    log!(message_severity, "{}\n", {
        let mut msg = format!(
            "\n{} [{} ({})] : {}",
            message_type,
            message_id_name,
            &message_id_number.to_string(),
            message
        );

        for (info_label, info) in additional_info.into_iter() {
            match info {
                Some(data) => {
                    msg = format!("{}\n{}: {}", msg, info_label, data);
                }
                None => {}
            }
        }

        msg
    });

    vk::FALSE
}

impl Instance {
    pub fn create(name: &str, version: u32) -> Self {
        // TODO: return errors instead of panic
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        let app_name = CString::new(name).unwrap();
        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::APPLICATION_INFO,
            p_next: ptr::null(),
            p_application_name: app_name.as_ptr(),
            application_version: version,
            p_engine_name: b"gfx-rs\0".as_ptr() as *const _,
            engine_version: 1,
            api_version: vk_make_version!(1, 0, 0),
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
                        CStr::from_ptr(inst_ext.extension_name.as_ptr()).to_bytes()
                            == ext.to_bytes()
                    })
                    .map(|_| ext)
                    .or_else(|| {
                        warn!("Unable to find extension: {}", ext.to_string_lossy());
                        None
                    })
            })
            .collect::<Vec<&CStr>>();

        // Check requested layers against the available layers
        let layers = LAYERS
            .iter()
            .filter_map(|&layer| {
                instance_layers
                    .iter()
                    .find(|inst_layer| unsafe {
                        CStr::from_ptr(inst_layer.layer_name.as_ptr()).to_bytes()
                            == layer.to_bytes()
                    })
                    .map(|_| layer)
                    .or_else(|| {
                        warn!("Unable to find layer: {}", layer.to_string_lossy());
                        None
                    })
            })
            .collect::<Vec<&CStr>>();

        let instance = {
            let cstrings = layers
                .iter()
                .chain(extensions.iter())
                .map(|&s| CString::from(s))
                .collect::<Vec<_>>();

            let str_pointers = cstrings.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();

            let create_info = vk::InstanceCreateInfo {
                s_type: vk::StructureType::INSTANCE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::InstanceCreateFlags::empty(),
                p_application_info: &app_info,
                enabled_layer_count: layers.len() as _,
                pp_enabled_layer_names: str_pointers.as_ptr(),
                enabled_extension_count: extensions.len() as _,
                pp_enabled_extension_names: str_pointers[layers.len()..].as_ptr(),
            };

            unsafe { entry.create_instance(&create_info, None) }
                .expect("Unable to create Vulkan instance")
        };

        #[cfg(debug_assertions)]
        let debug_messenger = {
            let ext = ext::DebugUtils::new(entry, &instance);
            let info = vk::DebugUtilsMessengerCreateInfoEXT {
                s_type: vk::StructureType::DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
                p_next: ptr::null(),
                flags: vk::DebugUtilsMessengerCreateFlagsEXT::empty(),
                message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::all(),
                message_type: vk::DebugUtilsMessageTypeFlagsEXT::all(),
                pfn_user_callback: Some(debug_utils_messenger_callback),
                p_user_data: ptr::null_mut(),
            };
            let handle = unsafe { ext.create_debug_utils_messenger(&info, None) }.unwrap();
            Some((ext, handle))
        };
        #[cfg(not(debug_assertions))]
        let debug_messenger = None;

        Instance {
            raw: Arc::new(RawInstance(instance, debug_messenger)),
            extensions,
        }
    }
}

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        let devices = unsafe { self.raw.0.enumerate_physical_devices() }
            .expect("Unable to enumerate adapters");

        devices
            .into_iter()
            .map(|device| {
                let properties = unsafe { self.raw.0.get_physical_device_properties(device) };
                let info = hal::AdapterInfo {
                    name: unsafe {
                        CStr::from_ptr(properties.device_name.as_ptr())
                            .to_str()
                            .expect("Invalid UTF-8 string")
                            .to_owned()
                    },
                    vendor: properties.vendor_id as usize,
                    device: properties.device_id as usize,
                    device_type: match properties.device_type {
                        ash::vk::PhysicalDeviceType::OTHER => DeviceType::Other,
                        ash::vk::PhysicalDeviceType::INTEGRATED_GPU => DeviceType::IntegratedGpu,
                        ash::vk::PhysicalDeviceType::DISCRETE_GPU => DeviceType::DiscreteGpu,
                        ash::vk::PhysicalDeviceType::VIRTUAL_GPU => DeviceType::VirtualGpu,
                        ash::vk::PhysicalDeviceType::CPU => DeviceType::Cpu,
                        _ => DeviceType::Other,
                    },
                };
                let physical_device = PhysicalDevice {
                    instance: self.raw.clone(),
                    handle: device,
                    properties,
                };
                let queue_families = unsafe {
                    self.raw
                        .0
                        .get_physical_device_queue_family_properties(device)
                        .into_iter()
                        .enumerate()
                        .map(|(i, properties)| QueueFamily {
                            properties,
                            device,
                            index: i as u32,
                        })
                        .collect()
                };

                hal::Adapter {
                    info,
                    physical_device,
                    queue_families,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
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
    fn id(&self) -> queue::QueueFamilyId {
        queue::QueueFamilyId(self.index as _)
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PhysicalDevice {
    #[derivative(Debug = "ignore")]
    instance: Arc<RawInstance>,
    handle: vk::PhysicalDevice,
    properties: vk::PhysicalDeviceProperties,
}

impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    unsafe fn open(
        &self,
        families: &[(&QueueFamily, &[hal::QueuePriority])],
        requested_features: Features,
    ) -> Result<hal::Gpu<Backend>, DeviceCreationError> {
        let family_infos = families
            .iter()
            .map(|&(family, priorities)| vk::DeviceQueueCreateInfo {
                s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::DeviceQueueCreateFlags::empty(),
                queue_family_index: family.index,
                queue_count: priorities.len() as _,
                p_queue_priorities: priorities.as_ptr(),
            })
            .collect::<Vec<_>>();

        if !self.features().contains(requested_features) {
            return Err(DeviceCreationError::MissingFeature);
        }

        let enabled_features = conv::map_device_features(requested_features);

        // Create device
        let device_raw = {
            let cstrings = DEVICE_EXTENSIONS
                .iter()
                .map(|&s| CString::from(s))
                .collect::<Vec<_>>();

            let str_pointers = cstrings.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();

            let info = vk::DeviceCreateInfo {
                s_type: vk::StructureType::DEVICE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::DeviceCreateFlags::empty(),
                queue_create_info_count: family_infos.len() as u32,
                p_queue_create_infos: family_infos.as_ptr(),
                enabled_layer_count: 0,
                pp_enabled_layer_names: ptr::null(),
                enabled_extension_count: str_pointers.len() as u32,
                pp_enabled_extension_names: str_pointers.as_ptr(),
                p_enabled_features: &enabled_features,
            };

            self.instance
                .0
                .create_device(self.handle, &info, None)
                .map_err(Into::<result::Error>::into)
                .map_err(Into::<DeviceCreationError>::into)?
        };

        let swapchain_fn = vk::KhrSwapchainFn::load(|name| {
            mem::transmute(
                self.instance
                    .0
                    .get_device_proc_addr(device_raw.handle(), name.as_ptr()),
            )
        });

        let device = Device {
            raw: Arc::new(RawDevice(device_raw, requested_features)),
        };

        let device_arc = device.raw.clone();
        let queues = families
            .into_iter()
            .map(|&(family, ref priorities)| {
                let family_index = family.index;
                let mut family_raw = hal::backend::RawQueueGroup::new(family.clone());
                for id in 0..priorities.len() {
                    let queue_raw = device_arc.0.get_device_queue(family_index, id as _);
                    family_raw.add_queue(CommandQueue {
                        raw: Arc::new(queue_raw),
                        device: device_arc.clone(),
                        swapchain_fn: swapchain_fn.clone(),
                    });
                }
                family_raw
            })
            .collect();

        Ok(hal::Gpu {
            device,
            queues: queue::Queues::new(queues),
        })
    }

    fn format_properties(&self, format: Option<format::Format>) -> format::Properties {
        let properties = unsafe {
            self.instance.0.get_physical_device_format_properties(
                self.handle,
                format.map_or(vk::Format::UNDEFINED, conv::map_format),
            )
        };

        format::Properties {
            linear_tiling: conv::map_image_features(properties.linear_tiling_features),
            optimal_tiling: conv::map_image_features(properties.optimal_tiling_features),
            buffer_features: conv::map_buffer_features(properties.buffer_features),
        }
    }

    fn image_format_properties(
        &self,
        format: format::Format,
        dimensions: u8,
        tiling: image::Tiling,
        usage: image::Usage,
        view_caps: image::ViewCapabilities,
    ) -> Option<image::FormatProperties> {
        let format_properties = unsafe {
            self.instance.0.get_physical_device_image_format_properties(
                self.handle,
                conv::map_format(format),
                match dimensions {
                    1 => vk::ImageType::TYPE_1D,
                    2 => vk::ImageType::TYPE_2D,
                    3 => vk::ImageType::TYPE_3D,
                    _ => panic!("Unexpected image dimensionality: {}", dimensions),
                },
                conv::map_tiling(tiling),
                conv::map_image_usage(usage),
                conv::map_view_capabilities(view_caps),
            )
        };

        match format_properties {
            Ok(props) => Some(image::FormatProperties {
                max_extent: image::Extent {
                    width: props.max_extent.width,
                    height: props.max_extent.height,
                    depth: props.max_extent.depth,
                },
                max_levels: props.max_mip_levels as _,
                max_layers: props.max_array_layers as _,
                sample_count_mask: props.sample_counts.as_raw() as _,
                max_resource_size: props.max_resource_size as _,
            }),
            Err(vk::Result::ERROR_FORMAT_NOT_SUPPORTED) => None,
            Err(other) => {
                error!("Unexpected error in `image_format_properties`: {:?}", other);
                None
            }
        }
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        let mem_properties = unsafe {
            self.instance
                .0
                .get_physical_device_memory_properties(self.handle)
        };
        let memory_heaps = mem_properties.memory_heaps[..mem_properties.memory_heap_count as usize]
            .iter()
            .map(|mem| mem.size)
            .collect();
        let memory_types = mem_properties.memory_types[..mem_properties.memory_type_count as usize]
            .iter()
            .map(|mem| {
                use memory::Properties;
                let mut type_flags = Properties::empty();

                if mem
                    .property_flags
                    .intersects(vk::MemoryPropertyFlags::DEVICE_LOCAL)
                {
                    type_flags |= Properties::DEVICE_LOCAL;
                }
                if mem
                  .property_flags
                  .intersects(vk::MemoryPropertyFlags::HOST_VISIBLE)
                {
                    type_flags |= Properties::CPU_VISIBLE;
                }
                if mem
                    .property_flags
                    .intersects(vk::MemoryPropertyFlags::HOST_COHERENT)
                {
                    type_flags |= Properties::COHERENT;
                }
                if mem
                    .property_flags
                    .intersects(vk::MemoryPropertyFlags::HOST_CACHED)
                {
                    type_flags |= Properties::CPU_CACHED;
                }
                if mem
                    .property_flags
                    .intersects(vk::MemoryPropertyFlags::LAZILY_ALLOCATED)
                {
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

    fn features(&self) -> Features {
        // see https://github.com/gfx-rs/gfx/issues/1930
        let is_windows_intel_dual_src_bug = cfg!(windows)
            && self.properties.vendor_id == info::intel::VENDOR
            && (self.properties.device_id & info::intel::DEVICE_KABY_LAKE_MASK
                == info::intel::DEVICE_KABY_LAKE_MASK
                || self.properties.device_id & info::intel::DEVICE_SKY_LAKE_MASK
                    == info::intel::DEVICE_SKY_LAKE_MASK);

        let features = unsafe { self.instance.0.get_physical_device_features(self.handle) };
        let mut bits = Features::TRIANGLE_FAN |
            Features::SEPARATE_STENCIL_REF_VALUES |
            Features::SAMPLER_MIP_LOD_BIAS;

        if features.robust_buffer_access != 0 {
            bits |= Features::ROBUST_BUFFER_ACCESS;
        }
        if features.full_draw_index_uint32 != 0 {
            bits |= Features::FULL_DRAW_INDEX_U32;
        }
        if features.image_cube_array != 0 {
            bits |= Features::IMAGE_CUBE_ARRAY;
        }
        if features.independent_blend != 0 {
            bits |= Features::INDEPENDENT_BLENDING;
        }
        if features.geometry_shader != 0 {
            bits |= Features::GEOMETRY_SHADER;
        }
        if features.tessellation_shader != 0 {
            bits |= Features::TESSELLATION_SHADER;
        }
        if features.sample_rate_shading != 0 {
            bits |= Features::SAMPLE_RATE_SHADING;
        }
        if features.dual_src_blend != 0 && !is_windows_intel_dual_src_bug {
            bits |= Features::DUAL_SRC_BLENDING;
        }
        if features.logic_op != 0 {
            bits |= Features::LOGIC_OP;
        }
        if features.multi_draw_indirect != 0 {
            bits |= Features::MULTI_DRAW_INDIRECT;
        }
        if features.draw_indirect_first_instance != 0 {
            bits |= Features::DRAW_INDIRECT_FIRST_INSTANCE;
        }
        if features.depth_clamp != 0 {
            bits |= Features::DEPTH_CLAMP;
        }
        if features.depth_bias_clamp != 0 {
            bits |= Features::DEPTH_BIAS_CLAMP;
        }
        if features.fill_mode_non_solid != 0 {
            bits |= Features::NON_FILL_POLYGON_MODE;
        }
        if features.depth_bounds != 0 {
            bits |= Features::DEPTH_BOUNDS;
        }
        if features.wide_lines != 0 {
            bits |= Features::LINE_WIDTH;
        }
        if features.large_points != 0 {
            bits |= Features::POINT_SIZE;
        }
        if features.alpha_to_one != 0 {
            bits |= Features::ALPHA_TO_ONE;
        }
        if features.multi_viewport != 0 {
            bits |= Features::MULTI_VIEWPORTS;
        }
        if features.sampler_anisotropy != 0 {
            bits |= Features::SAMPLER_ANISOTROPY;
        }
        if features.texture_compression_etc2 != 0 {
            bits |= Features::FORMAT_ETC2;
        }
        if features.texture_compression_astc_ldr != 0 {
            bits |= Features::FORMAT_ASTC_LDR;
        }
        if features.texture_compression_bc != 0 {
            bits |= Features::FORMAT_BC;
        }
        if features.occlusion_query_precise != 0 {
            bits |= Features::PRECISE_OCCLUSION_QUERY;
        }
        if features.pipeline_statistics_query != 0 {
            bits |= Features::PIPELINE_STATISTICS_QUERY;
        }
        if features.vertex_pipeline_stores_and_atomics != 0 {
            bits |= Features::VERTEX_STORES_AND_ATOMICS;
        }
        if features.fragment_stores_and_atomics != 0 {
            bits |= Features::FRAGMENT_STORES_AND_ATOMICS;
        }
        //TODO: cover more features

        bits
    }

    fn limits(&self) -> Limits {
        let limits = &self.properties.limits;
        let max_group_count = limits.max_compute_work_group_count;
        let max_group_size = limits.max_compute_work_group_size;

        Limits {
            max_image_1d_size: limits.max_image_dimension1_d,
            max_image_2d_size: limits.max_image_dimension2_d,
            max_image_3d_size: limits.max_image_dimension3_d,
            max_image_cube_size: limits.max_image_dimension_cube,
            max_image_array_layers: limits.max_image_array_layers as _,
            max_texel_elements: limits.max_texel_buffer_elements as _,
            max_patch_size: limits.max_tessellation_patch_size as PatchSize,
            max_viewports: limits.max_viewports as _,
            max_viewport_dimensions: limits.max_viewport_dimensions,
            max_framebuffer_extent: image::Extent {
                width: limits.max_framebuffer_width,
                height: limits.max_framebuffer_height,
                depth: limits.max_framebuffer_layers,
            },
            max_compute_work_group_count: [
                max_group_count[0] as _,
                max_group_count[1] as _,
                max_group_count[2] as _,
            ],
            max_compute_work_group_size: [
                max_group_size[0] as _,
                max_group_size[1] as _,
                max_group_size[2] as _,
            ],
            max_vertex_input_attributes: limits.max_vertex_input_attributes as _,
            max_vertex_input_bindings: limits.max_vertex_input_bindings as _,
            max_vertex_input_attribute_offset: limits.max_vertex_input_attribute_offset as _,
            max_vertex_input_binding_stride: limits.max_vertex_input_binding_stride as _,
            max_vertex_output_components: limits.max_vertex_output_components as _,
            optimal_buffer_copy_offset_alignment: limits.optimal_buffer_copy_offset_alignment as _,
            optimal_buffer_copy_pitch_alignment: limits.optimal_buffer_copy_row_pitch_alignment as _,
            min_texel_buffer_offset_alignment: limits.min_texel_buffer_offset_alignment as _,
            min_uniform_buffer_offset_alignment: limits.min_uniform_buffer_offset_alignment as _,
            min_storage_buffer_offset_alignment: limits.min_storage_buffer_offset_alignment as _,
            framebuffer_color_samples_count: limits.framebuffer_color_sample_counts.as_raw() as _,
            framebuffer_depth_samples_count: limits.framebuffer_depth_sample_counts.as_raw() as _,
            framebuffer_stencil_samples_count: limits.framebuffer_stencil_sample_counts.as_raw()
                as _,
            max_color_attachments: limits.max_color_attachments as _,
            buffer_image_granularity: limits.buffer_image_granularity,
            non_coherent_atom_size: limits.non_coherent_atom_size as _,
            max_sampler_anisotropy: limits.max_sampler_anisotropy,
            min_vertex_input_binding_stride_alignment: 1,
            .. Limits::default() //TODO: please halp
        }
    }

    fn is_valid_cache(&self, cache: &[u8]) -> bool {
        assert!(cache.len() > 16 + vk::UUID_SIZE);
        let cache_info: &[u32] = unsafe { slice::from_raw_parts(cache as *const _ as *const _, 4) };

        // header length
        if cache_info[0] <= 0 {
            warn!("Bad header length {:?}", cache_info[0]);
            return false;
        }

        // cache header version
        if cache_info[1] != vk::PipelineCacheHeaderVersion::ONE.as_raw() as u32 {
            warn!("Unsupported cache header version: {:?}", cache_info[1]);
            return false;
        }

        // vendor id
        if cache_info[2] != self.properties.vendor_id {
            warn!(
                "Vendor ID mismatch. Device: {:?}, cache: {:?}.",
                self.properties.vendor_id, cache_info[2],
            );
            return false;
        }

        // device id
        if cache_info[3] != self.properties.device_id {
            warn!(
                "Device ID mismatch. Device: {:?}, cache: {:?}.",
                self.properties.device_id, cache_info[3],
            );
            return false;
        }

        if self.properties.pipeline_cache_uuid != cache[16..16 + vk::UUID_SIZE] {
            warn!(
                "Pipeline cache UUID mismatch. Device: {:?}, cache: {:?}.",
                self.properties.pipeline_cache_uuid,
                &cache[16..16 + vk::UUID_SIZE],
            );
            return false;
        }
        true
    }
}

#[doc(hidden)]
pub struct RawDevice(pub ash::Device, Features);
impl fmt::Debug for RawDevice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RawDevice") // TODO: Real Debug impl
    }
}
impl Drop for RawDevice {
    fn drop(&mut self) {
        unsafe {
            self.0.destroy_device(None);
        }
    }
}

// Need to explicitly synchronize on submission and present.
pub type RawCommandQueue = Arc<vk::Queue>;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct CommandQueue {
    raw: RawCommandQueue,
    device: Arc<RawDevice>,
    #[derivative(Debug = "ignore")]
    swapchain_fn: vk::KhrSwapchainFn,
}

impl hal::queue::RawCommandQueue<Backend> for CommandQueue {
    unsafe fn submit<'a, T, Ic, S, Iw, Is>(
        &mut self,
        submission: hal::queue::Submission<Ic, Iw, Is>,
        fence: Option<&native::Fence>,
    ) where
        T: 'a + Borrow<command::CommandBuffer>,
        Ic: IntoIterator<Item = &'a T>,
        S: 'a + Borrow<native::Semaphore>,
        Iw: IntoIterator<Item = (&'a S, PipelineStage)>,
        Is: IntoIterator<Item = &'a S>,
    {
        //TODO: avoid heap allocations
        let mut waits = Vec::new();
        let mut stages = Vec::new();

        let buffers = submission
            .command_buffers
            .into_iter()
            .map(|cmd| cmd.borrow().raw)
            .collect::<Vec<_>>();
        for (semaphore, stage) in submission.wait_semaphores {
            waits.push(semaphore.borrow().0);
            stages.push(conv::map_pipeline_stage(stage));
        }
        let signals = submission
            .signal_semaphores
            .into_iter()
            .map(|semaphore| semaphore.borrow().0)
            .collect::<Vec<_>>();

        let info = vk::SubmitInfo {
            s_type: vk::StructureType::SUBMIT_INFO,
            p_next: ptr::null(),
            wait_semaphore_count: waits.len() as u32,
            p_wait_semaphores: waits.as_ptr(),
            // If count is zero, AMD driver crashes if nullptr is not set for stage masks
            p_wait_dst_stage_mask: if stages.is_empty() {
                ptr::null()
            } else {
                stages.as_ptr()
            },
            command_buffer_count: buffers.len() as u32,
            p_command_buffers: buffers.as_ptr(),
            signal_semaphore_count: signals.len() as u32,
            p_signal_semaphores: signals.as_ptr(),
        };

        let fence_raw = fence.map(|fence| fence.0).unwrap_or(vk::Fence::null());

        let result = self.device.0.queue_submit(*self.raw, &[info], fence_raw);
        assert_eq!(Ok(()), result);
    }

    unsafe fn present<'a, W, Is, S, Iw>(
        &mut self,
        swapchains: Is,
        wait_semaphores: Iw,
    ) -> Result<Option<Suboptimal>, PresentError>
    where
        W: 'a + Borrow<window::Swapchain>,
        Is: IntoIterator<Item = (&'a W, SwapImageIndex)>,
        S: 'a + Borrow<native::Semaphore>,
        Iw: IntoIterator<Item = &'a S>,
    {
        let semaphores = wait_semaphores
            .into_iter()
            .map(|sem| sem.borrow().0)
            .collect::<Vec<_>>();

        let mut frames = Vec::new();
        let mut vk_swapchains = Vec::new();
        for (swapchain, index) in swapchains {
            vk_swapchains.push(swapchain.borrow().raw);
            frames.push(index);
        }

        let info = vk::PresentInfoKHR {
            s_type: vk::StructureType::PRESENT_INFO_KHR,
            p_next: ptr::null(),
            wait_semaphore_count: semaphores.len() as _,
            p_wait_semaphores: semaphores.as_ptr(),
            swapchain_count: vk_swapchains.len() as _,
            p_swapchains: vk_swapchains.as_ptr(),
            p_image_indices: frames.as_ptr(),
            p_results: ptr::null_mut(),
        };

        match self.swapchain_fn.queue_present_khr(*self.raw, &info) {
            vk::Result::SUCCESS => Ok(None),
            vk::Result::SUBOPTIMAL_KHR => Ok(Some(Suboptimal)),
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => Err(PresentError::OutOfMemory(OutOfMemory::OutOfHostMemory)),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => Err(PresentError::OutOfMemory(OutOfMemory::OutOfDeviceMemory)),
            vk::Result::ERROR_DEVICE_LOST => Err(PresentError::DeviceLost(DeviceLost)),
            vk::Result::ERROR_OUT_OF_DATE_KHR => Err(PresentError::OutOfDate),
            vk::Result::ERROR_SURFACE_LOST_KHR => Err(PresentError::SurfaceLost(SurfaceLost)),
            _ => panic!("Failed to present frame"),
        }
    }

    fn wait_idle(&self) -> Result<(), HostExecutionError> {
        unsafe {
            self.device
                .0
                .queue_wait_idle(*self.raw)
                .map_err(From::from)
                .map_err(From::<result::Error>::from) // HostExecutionError
        }
    }
}

#[derive(Debug)]
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

    type Memory = native::Memory;
    type CommandPool = pool::RawCommandPool;

    type ShaderModule = native::ShaderModule;
    type RenderPass = native::RenderPass;
    type Framebuffer = native::Framebuffer;

    type Buffer = native::Buffer;
    type BufferView = native::BufferView;
    type Image = native::Image;
    type ImageView = native::ImageView;
    type Sampler = native::Sampler;

    type ComputePipeline = native::ComputePipeline;
    type GraphicsPipeline = native::GraphicsPipeline;
    type PipelineLayout = native::PipelineLayout;
    type PipelineCache = native::PipelineCache;
    type DescriptorSetLayout = native::DescriptorSetLayout;
    type DescriptorPool = native::DescriptorPool;
    type DescriptorSet = native::DescriptorSet;

    type Fence = native::Fence;
    type Semaphore = native::Semaphore;
    type QueryPool = native::QueryPool;
}
