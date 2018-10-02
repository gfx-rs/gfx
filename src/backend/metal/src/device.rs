use {
    AsNative, Backend, PrivateCapabilities, QueueFamily, ResourceIndex, OnlineRecording,
    Shared, Surface, Swapchain, VisibilityShared,
    validate_line_width,
};
use {conversions as conv, command, native as n};
use internal::{Channel, FastStorageMap};
use native;
use range_alloc::RangeAllocator;

use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::hash_map::Entry;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;
use std::{cmp, iter, mem, ptr, slice, thread, time};

use hal::{self, error, image, pass, format, mapping, memory, buffer, pso, query};
use hal::device::{BindError, OutOfMemory, FramebufferError, ShaderError};
use hal::memory::Properties;
use hal::pool::CommandPoolCreateFlags;
use hal::queue::{QueueFamilyId, Queues};
use hal::range::RangeArg;

use cocoa::foundation::{NSRange, NSUInteger, NSInteger};
use foreign_types::ForeignType;
use metal::{self,
    MTLFeatureSet, MTLLanguageVersion, MTLArgumentAccess, MTLDataType, MTLPrimitiveType, MTLPrimitiveTopologyClass,
    MTLCPUCacheMode, MTLStorageMode, MTLResourceOptions,
    MTLVertexStepFunction, MTLSamplerBorderColor, MTLSamplerMipFilter, MTLTextureType,
    CaptureManager
};
use objc::rc::autoreleasepool;
use objc::runtime::{BOOL, NO, Object};
use parking_lot::Mutex;
use spirv_cross::{msl, spirv, ErrorCode as SpirvErrorCode};


const RESOURCE_HEAP_SUPPORT: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v3,
    MTLFeatureSet::iOS_GPUFamily2_v3,
    MTLFeatureSet::iOS_GPUFamily3_v2,
    MTLFeatureSet::tvOS_GPUFamily1_v2,
];

const ARGUMENT_BUFFER_SUPPORT: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v4,
    MTLFeatureSet::tvOS_GPUFamily1_v3,
    MTLFeatureSet::macOS_GPUFamily1_v3,
];

const ASTC_PIXEL_FORMAT_FEATURES: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily2_v1,
    MTLFeatureSet::iOS_GPUFamily2_v2,
    MTLFeatureSet::iOS_GPUFamily3_v1,
    MTLFeatureSet::iOS_GPUFamily2_v3,
    MTLFeatureSet::iOS_GPUFamily3_v2,
    MTLFeatureSet::iOS_GPUFamily2_v4,
    MTLFeatureSet::iOS_GPUFamily3_v3,
    MTLFeatureSet::iOS_GPUFamily4_v1,
    MTLFeatureSet::tvOS_GPUFamily1_v1,
    MTLFeatureSet::tvOS_GPUFamily1_v2,
    MTLFeatureSet::tvOS_GPUFamily1_v3,
    MTLFeatureSet::tvOS_GPUFamily2_v1,
];

const R8UNORM_SRGB_ALL: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily3_v1,
    MTLFeatureSet::iOS_GPUFamily2_v3,
    MTLFeatureSet::iOS_GPUFamily3_v2,
    MTLFeatureSet::iOS_GPUFamily2_v4,
    MTLFeatureSet::iOS_GPUFamily3_v3,
    MTLFeatureSet::iOS_GPUFamily4_v1,
    MTLFeatureSet::tvOS_GPUFamily1_v2,
    MTLFeatureSet::tvOS_GPUFamily1_v3,
    MTLFeatureSet::tvOS_GPUFamily2_v1,
];

const R8SNORM_NO_RESOLVE: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v1,
    MTLFeatureSet::iOS_GPUFamily1_v2,
    MTLFeatureSet::iOS_GPUFamily1_v3,
    MTLFeatureSet::iOS_GPUFamily1_v4,
];

const RG8UNORM_SRGB_NO_WRITE: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v1,
    MTLFeatureSet::iOS_GPUFamily2_v1,
    MTLFeatureSet::iOS_GPUFamily1_v2,
    MTLFeatureSet::iOS_GPUFamily2_v2,
    MTLFeatureSet::iOS_GPUFamily1_v3,
    MTLFeatureSet::iOS_GPUFamily1_v4,
    MTLFeatureSet::tvOS_GPUFamily1_v1,
];

const RG8SNORM_NO_RESOLVE: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v1,
    MTLFeatureSet::iOS_GPUFamily1_v2,
    MTLFeatureSet::iOS_GPUFamily1_v3,
    MTLFeatureSet::iOS_GPUFamily1_v4,
];

const RGBA8_SRGB: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily3_v1,
    MTLFeatureSet::iOS_GPUFamily2_v3,
    MTLFeatureSet::iOS_GPUFamily3_v2,
    MTLFeatureSet::iOS_GPUFamily2_v4,
    MTLFeatureSet::iOS_GPUFamily3_v3,
    MTLFeatureSet::iOS_GPUFamily4_v1,
    MTLFeatureSet::tvOS_GPUFamily1_v2,
    MTLFeatureSet::tvOS_GPUFamily1_v3,
    MTLFeatureSet::tvOS_GPUFamily2_v1,
];

const RGB10A2UNORM_ALL: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily3_v1,
    MTLFeatureSet::iOS_GPUFamily3_v2,
    MTLFeatureSet::iOS_GPUFamily3_v3,
    MTLFeatureSet::iOS_GPUFamily4_v1,
    MTLFeatureSet::tvOS_GPUFamily2_v1,
    MTLFeatureSet::macOS_GPUFamily1_v1,
    MTLFeatureSet::macOS_GPUFamily1_v2,
    MTLFeatureSet::macOS_GPUFamily1_v3,
];

const RGB10A2UINT_COLOR_WRITE: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily3_v1,
    MTLFeatureSet::iOS_GPUFamily3_v2,
    MTLFeatureSet::iOS_GPUFamily3_v3,
    MTLFeatureSet::iOS_GPUFamily4_v1,
    MTLFeatureSet::tvOS_GPUFamily2_v1,
    MTLFeatureSet::macOS_GPUFamily1_v1,
    MTLFeatureSet::macOS_GPUFamily1_v2,
    MTLFeatureSet::macOS_GPUFamily1_v3,
];

const RG11B10FLOAT_ALL: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily3_v1,
    MTLFeatureSet::iOS_GPUFamily3_v2,
    MTLFeatureSet::iOS_GPUFamily3_v3,
    MTLFeatureSet::iOS_GPUFamily4_v1,
    MTLFeatureSet::tvOS_GPUFamily2_v1,
    MTLFeatureSet::macOS_GPUFamily1_v1,
    MTLFeatureSet::macOS_GPUFamily1_v2,
    MTLFeatureSet::macOS_GPUFamily1_v3,
];

const RGB9E5FLOAT_ALL: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily3_v1,
    MTLFeatureSet::iOS_GPUFamily3_v2,
    MTLFeatureSet::iOS_GPUFamily3_v3,
    MTLFeatureSet::iOS_GPUFamily4_v1,
    MTLFeatureSet::tvOS_GPUFamily2_v1,
];

const BGR10A2_ALL: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v4,
    MTLFeatureSet::iOS_GPUFamily2_v4,
    MTLFeatureSet::iOS_GPUFamily3_v3,
    MTLFeatureSet::iOS_GPUFamily4_v1,
    MTLFeatureSet::tvOS_GPUFamily1_v3,
    MTLFeatureSet::tvOS_GPUFamily2_v1,
];

const BASE_INSTANCE_SUPPORT: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v4,
    MTLFeatureSet::iOS_GPUFamily3_v1,
];

const DUAL_SOURCE_BLEND_SUPPORT: &[MTLFeatureSet] = &[
    MTLFeatureSet::iOS_GPUFamily1_v4,
    MTLFeatureSet::tvOS_GPUFamily1_v3,
    MTLFeatureSet::macOS_GPUFamily1_v2,
];

const PUSH_CONSTANTS_DESC_SET: u32 = !0;
const PUSH_CONSTANTS_DESC_BINDING: u32 = 0;


/// Emit error during shader module parsing.
fn gen_parse_error(err: SpirvErrorCode) -> ShaderError {
    let msg = match err {
        SpirvErrorCode::CompilationError(msg) => msg,
        SpirvErrorCode::Unhandled => "Unknown parse error".into(),
    };
    ShaderError::CompilationFailed(msg)
}

#[derive(Clone, Debug)]
enum FunctionError {
    InvalidEntryPoint,
    MissingRequiredSpecialization,
    BadSpecialization,
}

fn get_final_function(
    library: &metal::LibraryRef, entry: &str, specialization: pso::Specialization
) -> Result<metal::Function, FunctionError> {
    type MTLFunctionConstant = Object;

    let mut mtl_function = library
        .get_function(entry, None)
        .map_err(|e| {
            error!("Function retrieval error {:?}", e);
            FunctionError::InvalidEntryPoint
        })?;

    let dictionary = mtl_function.function_constants_dictionary();
    let count: NSUInteger = unsafe {
        msg_send![dictionary, count]
    };
    if count == 0 {
        return Ok(mtl_function)
    }

    let all_values: *mut Object = unsafe {
        msg_send![dictionary, allValues]
    };

    let constants = metal::FunctionConstantValues::new();
    for i in 0 .. count {
        let object: *mut MTLFunctionConstant = unsafe {
            msg_send![all_values, objectAtIndex: i]
        };
        let index: NSUInteger = unsafe {
            msg_send![object, index]
        };
        let required: BOOL = unsafe {
            msg_send![object, required]
        };
        match specialization.constants.iter().find(|c| c.id as NSUInteger == index) {
            Some(c) => unsafe {
                let ptr = &specialization.data[c.range.start as usize] as *const u8 as *const _;
                let ty: MTLDataType = msg_send![object, type];
                constants.set_constant_value_at_index(c.id as NSUInteger, ty, ptr);
            }
            None if required != NO => {
                //TODO: get name
                error!("Missing required specialization constant id {}", index);
                return Err(FunctionError::MissingRequiredSpecialization)
            }
            None => {}
        }
    }

    mtl_function = library
        .get_function(entry, Some(constants))
        .map_err(|e| {
            error!("Specialized function retrieval error {:?}", e);
            FunctionError::BadSpecialization
        })?;

    Ok(mtl_function)
}

impl VisibilityShared {
    fn are_available(&self, pool_base: query::Id, queries: &Range<query::Id>) -> bool {
        unsafe {
            let availability_ptr = ((self.buffer.contents() as *mut u8)
                .offset(self.availability_offset as isize) as *mut u32)
                .offset(pool_base as isize);
            queries.clone().all(|id| *availability_ptr.offset(id as isize) != 0)
        }
    }
}

//#[derive(Clone)]
pub struct Device {
    pub(crate) shared: Arc<Shared>,
    pub(crate) private_caps: PrivateCapabilities,
    memory_types: Vec<hal::MemoryType>,
    pub online_recording: OnlineRecording,
}
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        if cfg!(feature = "auto-capture") {
            info!("Metal capture stop");
            let shared_capture_manager = CaptureManager::shared();
            if let Some(default_capture_scope) = shared_capture_manager.default_capture_scope() {
                default_capture_scope.end_scope();
            }
            shared_capture_manager.stop_capture();
        }
    }
}

bitflags! {
    /// Memory type bits.
    struct MemoryTypes: u64 {
        const PRIVATE = 1<<0;
        const SHARED = 1<<1;
        const MANAGED_UPLOAD = 1<<2;
        const MANAGED_DOWNLOAD = 1<<3;
    }
}

impl MemoryTypes {
    fn describe(index: usize) -> (MTLStorageMode, MTLCPUCacheMode) {
        match Self::from_bits(1 << index).unwrap() {
            Self::PRIVATE          => (MTLStorageMode::Private, MTLCPUCacheMode::DefaultCache),
            Self::SHARED           => (MTLStorageMode::Shared,  MTLCPUCacheMode::DefaultCache),
            Self::MANAGED_UPLOAD   => (MTLStorageMode::Managed, MTLCPUCacheMode::WriteCombined),
            Self::MANAGED_DOWNLOAD => (MTLStorageMode::Managed, MTLCPUCacheMode::DefaultCache),
            _ => unreachable!()
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSOperatingSystemVersion {
    major: NSInteger,
    minor: NSInteger,
    patch: NSInteger,
}

pub struct PhysicalDevice {
    shared: Arc<Shared>,
    memory_types: Vec<hal::MemoryType>,
    pub(crate) private_caps: PrivateCapabilities,
}
unsafe impl Send for PhysicalDevice {}
unsafe impl Sync for PhysicalDevice {}

impl PhysicalDevice {
    fn supports_any(raw: &metal::DeviceRef, features_sets: &[MTLFeatureSet]) -> bool {
        features_sets.iter().cloned().any(|x| raw.supports_feature_set(x))
    }

    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        let device = shared.device.lock();

        let version: NSOperatingSystemVersion = unsafe {
            let process_info: *mut Object = msg_send![class!(NSProcessInfo), processInfo];
            msg_send![process_info, operatingSystemVersion]
        };

        let major = version.major as u32;
        let minor = version.minor as u32;
        let os_is_mac = device.supports_feature_set(MTLFeatureSet::macOS_GPUFamily1_v1);

        let private_caps = {
            PrivateCapabilities {
                os_is_mac,
                os_version: (major as u32, minor as u32),
                msl_version: if os_is_mac {
                    if PrivateCapabilities::version_at_least(major, minor, 10, 13) {
                        MTLLanguageVersion::V2_0
                    } else if PrivateCapabilities::version_at_least(major, minor, 10, 12) {
                        MTLLanguageVersion::V1_2
                    } else if PrivateCapabilities::version_at_least(major, minor, 10, 11) {
                        MTLLanguageVersion::V1_1
                    } else {
                        MTLLanguageVersion::V1_0
                    }
                } else if PrivateCapabilities::version_at_least(major, minor, 11, 0) {
                    MTLLanguageVersion::V2_0
                } else if PrivateCapabilities::version_at_least(major, minor, 10, 0) {
                    MTLLanguageVersion::V1_2
                } else if PrivateCapabilities::version_at_least(major, minor, 9, 0) {
                    MTLLanguageVersion::V1_1
                } else {
                    MTLLanguageVersion::V1_0
                },
                exposed_queues: 1,
                resource_heaps: Self::supports_any(&device, RESOURCE_HEAP_SUPPORT),
                argument_buffers: Self::supports_any(&device, ARGUMENT_BUFFER_SUPPORT) && false, //TODO
                shared_textures: !os_is_mac,
                base_instance: Self::supports_any(&device, BASE_INSTANCE_SUPPORT),
                dual_source_blending: Self::supports_any(&device, DUAL_SOURCE_BLEND_SUPPORT),
                low_power: !os_is_mac || device.is_low_power(),
                headless: os_is_mac && device.is_headless(),
                format_depth24_stencil8: os_is_mac && device.d24_s8_supported(),
                format_depth32_stencil8_filter: os_is_mac,
                format_depth32_stencil8_none: !os_is_mac,
                format_min_srgb_channels: if os_is_mac {4} else {1},
                format_b5: !os_is_mac,
                format_bc: os_is_mac,
                format_eac_etc: !os_is_mac,
                format_astc: Self::supports_any(&device, ASTC_PIXEL_FORMAT_FEATURES),
                format_r8unorm_srgb_all: Self::supports_any(&device, R8UNORM_SRGB_ALL),
                format_r8unorm_srgb_no_write: !Self::supports_any(&device, R8UNORM_SRGB_ALL) && !os_is_mac,
                format_r8snorm_all: !Self::supports_any(&device, R8SNORM_NO_RESOLVE),
                format_r16_norm_all: os_is_mac,
                format_rg8unorm_srgb_all: Self::supports_any(&device, RG8UNORM_SRGB_NO_WRITE),
                format_rg8unorm_srgb_no_write: !Self::supports_any(&device, RG8UNORM_SRGB_NO_WRITE) && !os_is_mac,
                format_rg8snorm_all: !Self::supports_any(&device, RG8SNORM_NO_RESOLVE),
                format_r32_all: !Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]),
                format_r32_no_write: Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]),
                format_r32float_no_write_no_filter: Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]) && !os_is_mac,
                format_r32float_no_filter: !Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]) && !os_is_mac,
                format_r32float_all: os_is_mac,
                format_rgba8_srgb_all: Self::supports_any(&device, RGBA8_SRGB),
                format_rgba8_srgb_no_write: !Self::supports_any(&device, RGBA8_SRGB),
                format_rgb10a2_unorm_all: Self::supports_any(&device, RGB10A2UNORM_ALL),
                format_rgb10a2_unorm_no_write: !Self::supports_any(&device, RGB10A2UNORM_ALL),
                format_rgb10a2_uint_color: !Self::supports_any(&device, RGB10A2UINT_COLOR_WRITE),
                format_rgb10a2_uint_color_write: Self::supports_any(&device, RGB10A2UINT_COLOR_WRITE),
                format_rg11b10_all: Self::supports_any(&device, RG11B10FLOAT_ALL),
                format_rg11b10_no_write: !Self::supports_any(&device, RG11B10FLOAT_ALL),
                format_rgb9e5_all: Self::supports_any(&device, RGB9E5FLOAT_ALL),
                format_rgb9e5_no_write: !Self::supports_any(&device, RGB9E5FLOAT_ALL) && !os_is_mac,
                format_rgb9e5_filter_only: os_is_mac,
                format_rg32_color: Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]),
                format_rg32_color_write: !Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]),
                format_rg32float_all: os_is_mac,
                format_rg32float_color_blend: Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]),
                format_rg32float_no_filter: !os_is_mac && !Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]),
                format_rgba32int_color: Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]),
                format_rgba32int_color_write: !Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]),
                format_rgba32float_color: Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]),
                format_rgba32float_color_write: !Self::supports_any(&device, &[MTLFeatureSet::iOS_GPUFamily1_v1, MTLFeatureSet::iOS_GPUFamily2_v1]) && !os_is_mac,
                format_rgba32float_all: os_is_mac,
                format_depth16unorm: Self::supports_any(&device, &[MTLFeatureSet::macOS_GPUFamily1_v2, MTLFeatureSet::macOS_GPUFamily1_v3]),
                format_depth32float_filter: Self::supports_any(&device, &[MTLFeatureSet::macOS_GPUFamily1_v1, MTLFeatureSet::macOS_GPUFamily1_v2, MTLFeatureSet::macOS_GPUFamily1_v3]),
                format_depth32float_none: !Self::supports_any(&device, &[MTLFeatureSet::macOS_GPUFamily1_v1, MTLFeatureSet::macOS_GPUFamily1_v2, MTLFeatureSet::macOS_GPUFamily1_v3]),
                format_bgr10a2_all: Self::supports_any(&device, BGR10A2_ALL),
                format_bgr10a2_no_write: !Self::supports_any(&device, &[MTLFeatureSet::macOS_GPUFamily1_v3]),
                max_buffers_per_stage: 31,
                max_textures_per_stage: if os_is_mac {128} else {31},
                max_samplers_per_stage: 16,
                buffer_alignment: if os_is_mac {256} else {64},
                max_buffer_size: if Self::supports_any(&device, &[MTLFeatureSet::macOS_GPUFamily1_v2, MTLFeatureSet::macOS_GPUFamily1_v3]) {
                    1 << 30 // 1GB on macOS 1.2 and up
                } else {
                    1 << 28 // 256MB otherwise
                },
                max_texture_size: 4096, //TODO
            }
        };

        let memory_types = if os_is_mac {
            vec![
                hal::MemoryType { // PRIVATE
                    properties: Properties::DEVICE_LOCAL,
                    heap_index: 0,
                },
                hal::MemoryType { // SHARED
                    properties: Properties::CPU_VISIBLE | Properties::COHERENT,
                    heap_index: 1,
                },
                hal::MemoryType { // MANAGED_UPLOAD
                    properties: Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE,
                    heap_index: 1,
                },
                hal::MemoryType { // MANAGED_DOWNLOAD
                    properties: Properties::DEVICE_LOCAL | Properties::CPU_VISIBLE | Properties::CPU_CACHED,
                    heap_index: 1,
                },
            ]
        } else {
            vec![
                hal::MemoryType { // PRIVATE
                    properties: Properties::DEVICE_LOCAL,
                    heap_index: 0,
                },
                hal::MemoryType { // SHARED
                    properties: Properties::CPU_VISIBLE | Properties::COHERENT,
                    heap_index: 1,
                },
            ]
        };
        PhysicalDevice {
            shared:  shared.clone(),
            memory_types,
            private_caps,
        }
    }
}

impl hal::PhysicalDevice<Backend> for PhysicalDevice {
    fn open(
        &self, families: &[(&QueueFamily, &[hal::QueuePriority])],
    ) -> Result<hal::Gpu<Backend>, error::DeviceCreationError> {
        // TODO: Handle opening a physical device multiple times
        assert_eq!(families.len(), 1);
        assert_eq!(families[0].1.len(), 1);
        let family = *families[0].0;
        let device = self.shared.device.lock();

        if cfg!(feature = "auto-capture") {
            info!("Metal capture start");
            let shared_capture_manager = CaptureManager::shared();
            let default_capture_scope = shared_capture_manager.new_capture_scope_with_device(&*device);
            shared_capture_manager.set_default_capture_scope(default_capture_scope);
            shared_capture_manager.start_capture_with_scope(&default_capture_scope);
            default_capture_scope.begin_scope();
        }

        let mut queue_group = hal::backend::RawQueueGroup::new(family);
        for _ in 0 .. self.private_caps.exposed_queues {
            queue_group.add_queue(command::CommandQueue::new(self.shared.clone()));
        }

        let device = Device {
            shared: self.shared.clone(),
            private_caps: self.private_caps.clone(),
            memory_types: self.memory_types.clone(),
            online_recording: OnlineRecording::default(),
        };

        Ok(hal::Gpu {
            device,
            queues: Queues::new(vec![queue_group]),
        })
    }

    fn format_properties(&self, format: Option<format::Format>) -> format::Properties {
        match format.and_then(|f| self.private_caps.map_format(f)) {
            Some(format) =>  {
                self.private_caps.map_format_properties(format)
            },
            None => format::Properties {
                linear_tiling: format::ImageFeature::empty(),
                optimal_tiling: format::ImageFeature::empty(),
                buffer_features: format::BufferFeature::empty(),
            },
        }
    }

    fn image_format_properties(
        &self, format: format::Format, dimensions: u8, tiling: image::Tiling,
        usage: image::Usage, view_caps: image::ViewCapabilities,
    ) -> Option<image::FormatProperties> {
        if let image::Tiling::Linear = tiling {
            let format_desc = format.surface_desc();
            let host_usage = image::Usage::TRANSFER_SRC | image::Usage::TRANSFER_DST;
            if dimensions != 2 ||
                !view_caps.is_empty() ||
                !host_usage.contains(usage) ||
                format_desc.aspects != format::Aspects::COLOR ||
                format_desc.is_compressed()
            {
                return None
            }
        }
        if dimensions == 1 && usage.intersects(image::Usage::COLOR_ATTACHMENT | image::Usage::DEPTH_STENCIL_ATTACHMENT) {
            // MTLRenderPassDescriptor texture must not be MTLTextureType1D
            return None;
        }
        if dimensions == 3 && view_caps.contains(image::ViewCapabilities::KIND_2D_ARRAY) {
            // Can't create 2D/2DArray views of 3D textures
            return None;
        }
        //TODO: actually query this data
        let max_dimension = 4096u32;
        let max_extent = image::Extent {
            width: max_dimension,
            height: if dimensions >= 2 { max_dimension } else { 1 },
            depth: if dimensions >= 3 { max_dimension } else { 1 },
        };

        self.private_caps.map_format(format).map(|_| image::FormatProperties {
            max_extent,
            max_levels: if dimensions == 1 { 1 } else { 12 },
            // 3D images enforce a single layer
            max_layers: if dimensions == 3 { 1 } else { 2048 },
            sample_count_mask: 0x1,
            //TODO: buffers and textures have separate limits
            // Max buffer size is determined by feature set
            // Max texture size does not appear to be documented publicly
            max_resource_size: self.private_caps.max_buffer_size as _,
        })
    }

    fn memory_properties(&self) -> hal::MemoryProperties {
        hal::MemoryProperties {
            memory_heaps: vec![
                !0, //TODO: private memory limits
                self.private_caps.max_buffer_size,
            ],
            memory_types: self.memory_types.to_vec(),
        }
    }

    fn features(&self) -> hal::Features {
        hal::Features::ROBUST_BUFFER_ACCESS |
        hal::Features::DRAW_INDIRECT_FIRST_INSTANCE |
        hal::Features::DEPTH_CLAMP |
        hal::Features::SAMPLER_ANISOTROPY |
        hal::Features::FORMAT_BC |
        hal::Features::PRECISE_OCCLUSION_QUERY |
        hal::Features::SHADER_STORAGE_BUFFER_ARRAY_DYNAMIC_INDEXING |
        hal::Features::VERTEX_STORES_AND_ATOMICS |
        hal::Features::FRAGMENT_STORES_AND_ATOMICS |
        if self.private_caps.dual_source_blending { hal::Features::DUAL_SRC_BLENDING } else { hal::Features::empty() }
    }

    fn limits(&self) -> hal::Limits {
        hal::Limits {
            max_texture_size: self.private_caps.max_texture_size as usize,
            max_texel_elements: (self.private_caps.max_texture_size * self.private_caps.max_texture_size) as usize,
            max_patch_size: 0, // No tessellation

            // Note: The maximum number of supported viewports and scissor rectangles varies by device.
            // TODO: read from Metal Feature Sets.
            max_viewports: 1,

            min_buffer_copy_offset_alignment: self.private_caps.buffer_alignment,
            min_buffer_copy_pitch_alignment: 4,
            min_texel_buffer_offset_alignment: self.private_caps.buffer_alignment,
            min_uniform_buffer_offset_alignment: self.private_caps.buffer_alignment,
            min_storage_buffer_offset_alignment: self.private_caps.buffer_alignment,

            max_compute_group_count: [16; 3], // TODO
            max_compute_group_size: [64; 3], // TODO

            max_vertex_input_attributes: 31,
            max_vertex_input_bindings: 31,
            max_vertex_input_attribute_offset: 255, // TODO
            max_vertex_input_binding_stride: 256, // TODO
            max_vertex_output_components: 16, // TODO

            framebuffer_color_samples_count: 0b101, // TODO
            framebuffer_depth_samples_count: 0b101, // TODO
            framebuffer_stencil_samples_count: 0b101, // TODO
            max_color_attachments: 1, // TODO

            // Note: we issue Metal buffer-to-buffer copies on memory flush/invalidate,
            // and those need to operate on sizes being multiples of 4.
            non_coherent_atom_size: 4,
            max_sampler_anisotropy: 16.,
        }
    }
}

pub struct LanguageVersion {
    pub major: u8,
    pub minor: u8,
}

impl LanguageVersion {
    pub fn new(major: u8, minor: u8) -> Self {
        LanguageVersion { major, minor }
    }
}

impl Device {
    fn _is_heap_coherent(&self, heap: &n::MemoryHeap) -> bool {
        match *heap {
            n::MemoryHeap::Private => false,
            n::MemoryHeap::Public(memory_type, _) => self.memory_types[memory_type.0].properties.contains(Properties::COHERENT),
            n::MemoryHeap::Native(ref heap) => heap.storage_mode() == MTLStorageMode::Shared,
        }
    }

    pub fn create_shader_library_from_file<P>(
        &self, _path: P,
    ) -> Result<n::ShaderModule, ShaderError> where P: AsRef<Path> {
        unimplemented!()
    }

    pub fn create_shader_library_from_source<S>(
        &self,
        source: S,
        version: LanguageVersion,
        rasterization_enabled: bool,
    ) -> Result<n::ShaderModule, ShaderError> where S: AsRef<str> {
        let options = metal::CompileOptions::new();
        let msl_version = match version {
            LanguageVersion { major: 1, minor: 0 } => MTLLanguageVersion::V1_0,
            LanguageVersion { major: 1, minor: 1 } => MTLLanguageVersion::V1_1,
            LanguageVersion { major: 1, minor: 2 } => MTLLanguageVersion::V1_2,
            LanguageVersion { major: 2, minor: 0 } => MTLLanguageVersion::V2_0,
            _ => return Err(ShaderError::CompilationFailed("shader model not supported".into()))
        };
        if msl_version > self.private_caps.msl_version {
            return Err(ShaderError::CompilationFailed("shader model too high".into()))
        }
        options.set_language_version(msl_version);

        self.shared.device
            .lock()
            .new_library_with_source(source.as_ref(), &options)
            .map(|library| n::ShaderModule::Compiled(n::ModuleInfo {
                library,
                entry_point_map: n::EntryPointMap::default(),
                rasterization_enabled,
            }))
            .map_err(|e| ShaderError::CompilationFailed(e.into()))
    }

    fn compile_shader_library(
        device: &Mutex<metal::Device>,
        raw_data: &[u8],
        compiler_options: &msl::CompilerOptions,
        msl_version: MTLLanguageVersion,
    ) -> Result<n::ModuleInfo, ShaderError> {
        // spec requires "codeSize must be a multiple of 4"
        assert_eq!(raw_data.len() & 3, 0);

        let module = spirv::Module::from_words(unsafe {
            slice::from_raw_parts(
                raw_data.as_ptr() as *const u32,
                raw_data.len() / mem::size_of::<u32>(),
            )
        });

        // now parse again using the new overrides
        let mut ast = spirv::Ast::<msl::Target>::parse(&module)
            .map_err(gen_parse_error)?;

        ast.set_compiler_options(compiler_options)
            .map_err(|err| {
                ShaderError::CompilationFailed(match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unexpected error".into(),
                })
            })?;

        let entry_points = ast.get_entry_points()
            .map_err(|err| {
                ShaderError::CompilationFailed(match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unexpected entry point error".into(),
                })
            })?;

        let shader_code = ast.compile()
            .map_err(|err| {
                ShaderError::CompilationFailed(match err {
                    SpirvErrorCode::CompilationError(msg) => msg,
                    SpirvErrorCode::Unhandled => "Unknown compile error".into(),
                })
            })?;

        let mut entry_point_map = n::EntryPointMap::default();
        for entry_point in entry_points {
            info!("Entry point {:?}", entry_point);
            let cleansed = ast.get_cleansed_entry_point_name(&entry_point.name, entry_point.execution_model)
                .map_err(|err| {
                    ShaderError::CompilationFailed(match err {
                        SpirvErrorCode::CompilationError(msg) => msg,
                        SpirvErrorCode::Unhandled => "Unknown compile error".into(),
                    })
                })?;
            entry_point_map.insert(entry_point.name, spirv::EntryPoint {
                name: cleansed,
                .. entry_point
            });
        }

        let rasterization_enabled = ast.is_rasterization_enabled()
            .map_err(|_| ShaderError::CompilationFailed("Unknown compile error".into()))?;

        // done
        debug!("SPIRV-Cross generated shader:\n{}", shader_code);

        let options = metal::CompileOptions::new();
        options.set_language_version(msl_version);

        let library = device
            .lock()
            .new_library_with_source(shader_code.as_ref(), &options)
            .map_err(|err| ShaderError::CompilationFailed(err.into()))?;

        Ok(n::ModuleInfo {
            library,
            entry_point_map,
            rasterization_enabled,
        })
    }

    fn load_shader(
        &self,
        ep: &pso::EntryPoint<Backend>,
        layout: &n::PipelineLayout,
        primitive_class: MTLPrimitiveTopologyClass,
        pipeline_cache: Option<&n::PipelineCache>,
    ) -> Result<(metal::Library, metal::Function, metal::MTLSize, bool), pso::CreationError> {
        let device = &self.shared.device;
        let msl_version = self.private_caps.msl_version;
        let module_map;
        let (info_owned, info_guard);

        let info = match *ep.module {
            n::ShaderModule::Compiled(ref info) => info,
            n::ShaderModule::Raw(ref data) => {
                let compiler_options = match primitive_class {
                    MTLPrimitiveTopologyClass::Point => &layout.shader_compiler_options_point,
                    _ => &layout.shader_compiler_options,
                };
                match pipeline_cache {
                    Some(cache) => {
                        module_map = cache.modules.get_or_create_with(compiler_options, || {
                            FastStorageMap::default()
                        });
                        info_guard = module_map.get_or_create_with(data, || {
                            Self::compile_shader_library(device, data, compiler_options, msl_version)
                                .unwrap()
                        });
                        &*info_guard
                    }
                    None => {
                        info_owned = Self::compile_shader_library(device, data, compiler_options, msl_version)
                            .map_err(|e| {
                                error!("Error compiling the shader {:?}", e);
                                pso::CreationError::Other
                            })?;
                        &info_owned
                    }
                }
            }
        };

        let lib = info.library.clone();
        let (name, wg_size) = match info.entry_point_map.get(ep.entry) {
            Some(p) => (p.name.as_str(), metal::MTLSize {
                width : p.work_group_size.x as _,
                height: p.work_group_size.y as _,
                depth : p.work_group_size.z as _,
            }),
            // this can only happen if the shader came directly from the user
            None => (ep.entry, metal::MTLSize { width: 0, height: 0, depth: 0 }),
        };
        let mtl_function = get_final_function(&lib, name, ep.specialization)
            .map_err(|e| {
                error!("Invalid shader entry point '{}': {:?}", name, e);
                pso::CreationError::Other
            })?;

        Ok((lib, mtl_function, wg_size, info.rasterization_enabled))
    }

    fn describe_argument(
        ty: pso::DescriptorType, index: pso::DescriptorBinding, count: usize
    ) -> metal::ArgumentDescriptor {
        let arg = metal::ArgumentDescriptor::new().to_owned();
        arg.set_array_length(count as _);

        match ty {
            pso::DescriptorType::Sampler => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Sampler);
                arg.set_index(index as _);
            }
            pso::DescriptorType::SampledImage => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Texture);
                arg.set_index(index as _);
            }
            pso::DescriptorType::UniformBuffer => {
                arg.set_access(MTLArgumentAccess::ReadOnly);
                arg.set_data_type(MTLDataType::Struct);
                arg.set_index(index as _);
            }
            pso::DescriptorType::StorageBuffer => {
                arg.set_access(MTLArgumentAccess::ReadWrite);
                arg.set_data_type(MTLDataType::Struct);
                arg.set_index(index as _);
            }
            _ => unimplemented!()
        }

        arg
    }
}

impl hal::Device<Backend> for Device {
    fn create_command_pool(
        &self, _family: QueueFamilyId, _flags: CommandPoolCreateFlags
    ) -> command::CommandPool {
        command::CommandPool::new(&self.shared, self.online_recording.clone())
    }

    fn destroy_command_pool(&self, mut pool: command::CommandPool) {
        use hal::pool::RawCommandPool;
        pool.reset();
    }

    fn create_render_pass<'a, IA, IS, ID>(
        &self,
        attachments: IA,
        subpasses: IS,
        _dependencies: ID,
    ) -> n::RenderPass
    where
        IA: IntoIterator,
        IA::Item: Borrow<pass::Attachment>,
        IS: IntoIterator,
        IS::Item: Borrow<pass::SubpassDesc<'a>>,
        ID: IntoIterator,
        ID::Item: Borrow<pass::SubpassDependency>,
    {
        let attachments: Vec<pass::Attachment> = attachments
            .into_iter()
            .map(|at| at.borrow().clone())
            .collect();

        let mut subpasses: Vec<n::Subpass> = subpasses
            .into_iter()
            .map(|sp| {
                let sub = sp.borrow();
                n::Subpass {
                    colors: sub.colors
                        .iter()
                        .map(|&(id, _)| (id, n::SubpassOps::empty()))
                        .collect(),
                    depth_stencil: sub.depth_stencil
                        .map(|&(id, _)| (id, n::SubpassOps::empty())),
                    inputs: sub.inputs
                        .iter()
                        .map(|&(id, _)| id)
                        .collect(),
                    target_formats: n::SubpassFormats {
                        colors: sub.colors
                            .iter()
                            .map(|&(id, _)| {
                                let format = attachments[id].format.expect("No color format provided");
                                let mtl_format = self.private_caps.map_format(format).expect("Unable to map color format!");
                                (mtl_format, Channel::from(format.base_format().1))
                            })
                            .collect(),
                        depth_stencil: sub.depth_stencil
                            .map(|&(id, _)| {
                                self.private_caps.map_format(
                                    attachments[id].format.expect("No depth-stencil format provided")
                                ).expect("Unable to map depth-stencil format!")
                            }),
                    },
                }
            })
            .collect();

        // sprinkle load operations
        // an attachment receives LOAD flag on a subpass if it's the first sub-pass that uses it
        let mut use_mask = 0u64;
        for sub in subpasses.iter_mut() {
            for &mut (id, ref mut ops) in sub.colors.iter_mut().chain(sub.depth_stencil.as_mut()) {
                if use_mask & 1 << id == 0 {
                    *ops |= n::SubpassOps::LOAD;
                    use_mask ^= 1 << id;
                }
            }
        }
        // sprinkle store operations
        // an attachment receives STORE flag on a subpass if it's the last sub-pass that uses it
        for sub in subpasses.iter_mut().rev() {
            for &mut (id, ref mut ops) in sub.colors.iter_mut().chain(sub.depth_stencil.as_mut()) {
                if use_mask & 1 << id != 0 {
                    *ops |= n::SubpassOps::STORE;
                    use_mask ^= 1 << id;
                }
            }
        }

        n::RenderPass {
            attachments,
            subpasses,
        }
    }

    fn create_pipeline_layout<IS, IR>(
        &self,
        set_layouts: IS,
        push_constant_ranges: IR,
    ) -> n::PipelineLayout
    where
        IS: IntoIterator,
        IS::Item: Borrow<n::DescriptorSetLayout>,
        IR: IntoIterator,
        IR::Item: Borrow<(pso::ShaderStageFlags, Range<u32>)>
    {
        let mut stage_infos = [
            (pso::ShaderStageFlags::VERTEX,   spirv::ExecutionModel::Vertex,    n::ResourceData::<ResourceIndex>::new()),
            (pso::ShaderStageFlags::FRAGMENT, spirv::ExecutionModel::Fragment,  n::ResourceData::<ResourceIndex>::new()),
            (pso::ShaderStageFlags::COMPUTE,  spirv::ExecutionModel::GlCompute, n::ResourceData::<ResourceIndex>::new()),
        ];
        let mut res_overrides = BTreeMap::new();
        let mut infos = Vec::new();

        for (set_index, set_layout) in set_layouts.into_iter().enumerate() {
            // remember where the resources for this set start at each shader stage
            let mut dynamic_buffers = Vec::new();
            let offsets = n::MultiStageResourceCounters {
                vs: stage_infos[0].2.clone(),
                ps: stage_infos[1].2.clone(),
                cs: stage_infos[2].2.clone(),
            };
            match *set_layout.borrow() {
                n::DescriptorSetLayout::Emulated(ref desc_layouts, _) => {
                    for layout in desc_layouts.iter() {
                        if layout.content.contains(n::DescriptorContent::DYNAMIC_BUFFER) {
                            dynamic_buffers.push(n::MultiStageData {
                                vs: if layout.stages.contains(pso::ShaderStageFlags::VERTEX) {
                                    stage_infos[0].2.buffers
                                } else {
                                    !0
                                },
                                ps: if layout.stages.contains(pso::ShaderStageFlags::FRAGMENT) {
                                    stage_infos[1].2.buffers
                                } else {
                                    !0
                                },
                                cs: if layout.stages.contains(pso::ShaderStageFlags::COMPUTE) {
                                    stage_infos[2].2.buffers
                                } else {
                                    !0
                                },
                            });
                        }
                        for &mut (stage_bit, stage, ref mut counters) in stage_infos.iter_mut() {
                            if !layout.stages.contains(stage_bit) {
                                continue
                            }
                            let res = msl::ResourceBinding {
                                buffer_id: if layout.content.contains(n::DescriptorContent::BUFFER) {
                                    counters.buffers += 1;
                                    (counters.buffers - 1) as _
                                } else { !0 },
                                texture_id: if layout.content.contains(n::DescriptorContent::TEXTURE) {
                                    counters.textures += 1;
                                    (counters.textures - 1) as _
                                } else { !0 },
                                sampler_id: if layout.content.contains(n::DescriptorContent::SAMPLER) {
                                    counters.samplers += 1;
                                    (counters.samplers - 1) as _
                                } else { !0 },
                                force_used: false,
                            };
                            if layout.array_index == 0 {
                                let location = msl::ResourceBindingLocation {
                                    stage,
                                    desc_set: set_index as _,
                                    binding: layout.binding,
                                };
                                res_overrides.insert(location, res);
                            }
                        }
                    }
                }
                n::DescriptorSetLayout::ArgumentBuffer(_, stage_flags) => {
                    for &mut (stage_bit, stage, ref mut counters) in stage_infos.iter_mut() {
                        if !stage_flags.contains(stage_bit) {
                            continue
                        }
                        let location = msl::ResourceBindingLocation {
                            stage,
                            desc_set: set_index as _,
                            binding: 0,
                        };
                        let res_binding = msl::ResourceBinding {
                            buffer_id: counters.buffers as _,
                            texture_id: !0,
                            sampler_id: !0,
                            force_used: false,
                        };
                        res_overrides.insert(location, res_binding);
                        counters.buffers += 1;
                    }
                }
            }

            infos.push(n::DescriptorSetInfo {
                offsets,
                dynamic_buffers,
            });
        }

        let mut pc_buffers = [None; 3];
        let mut pc_limits = [0u32; 3];
        for pcr in push_constant_ranges {
            let (flags, range) = pcr.borrow();
            for (limit, &(stage_bit, _, _)) in pc_limits.iter_mut().zip(&stage_infos) {
                if flags.contains(stage_bit) {
                    *limit = range.end.max(*limit);
                }
            }
        }

        for ((limit, ref mut buffer_index), &mut (_, stage, ref mut counters)) in pc_limits
            .iter()
            .zip(pc_buffers.iter_mut())
            .zip(stage_infos.iter_mut())
        {
            // handle the push constant buffer assignment and shader overrides
            if *limit != 0 {
                let index = counters.buffers;
                **buffer_index = Some(index);
                counters.buffers += 1;

                res_overrides.insert(
                    msl::ResourceBindingLocation {
                        stage,
                        desc_set: PUSH_CONSTANTS_DESC_SET,
                        binding: PUSH_CONSTANTS_DESC_BINDING,
                    },
                    msl::ResourceBinding {
                        buffer_id: index as _,
                        texture_id: !0,
                        sampler_id: !0,
                        force_used: false,
                    },
                );
            }
            // make sure we fit the limits
            assert!(counters.buffers <= self.private_caps.max_buffers_per_stage);
            assert!(counters.textures <= self.private_caps.max_textures_per_stage);
            assert!(counters.samplers <= self.private_caps.max_samplers_per_stage);
        }

        let mut shader_compiler_options = msl::CompilerOptions::default();
        shader_compiler_options.version = match self.private_caps.msl_version {
            MTLLanguageVersion::V1_0 => msl::Version::V1_0,
            MTLLanguageVersion::V1_1 => msl::Version::V1_1,
            MTLLanguageVersion::V1_2 => msl::Version::V1_2,
            MTLLanguageVersion::V2_0 => msl::Version::V2_0,
        };
        shader_compiler_options.enable_point_size_builtin = false;
        shader_compiler_options.resolve_specialized_array_lengths = true;
        shader_compiler_options.vertex.invert_y = true;
        shader_compiler_options.resource_binding_overrides = res_overrides;
        let mut shader_compiler_options_point = shader_compiler_options.clone();
        shader_compiler_options_point.enable_point_size_builtin = true;

        const LIMIT_MASK: u32 = 3;
        // round up the limits alignment to 4, so that it matches MTL compiler logic
        //TODO: figure out what and how exactly does the alignment. Clearly, it's not
        // straightforward, given that value of 2 stays non-aligned.
        for limit in &mut pc_limits {
            if *limit > LIMIT_MASK {
                *limit = (*limit + LIMIT_MASK) & !LIMIT_MASK;
            }
        }

        n::PipelineLayout {
            shader_compiler_options,
            shader_compiler_options_point,
            infos,
            total: n::MultiStageResourceCounters {
                vs: stage_infos[0].2.clone(),
                ps: stage_infos[1].2.clone(),
                cs: stage_infos[2].2.clone(),
            },
            push_constants: n::MultiStageData {
                vs: pc_buffers[0].map(|buffer_index| n::PushConstantInfo {
                    count: pc_limits[0],
                    buffer_index,
                }),
                ps: pc_buffers[1].map(|buffer_index| n::PushConstantInfo {
                    count: pc_limits[1],
                    buffer_index,
                }),
                cs: pc_buffers[2].map(|buffer_index| n::PushConstantInfo {
                    count: pc_limits[2],
                    buffer_index,
                }),
            },
            total_push_constants: pc_limits[0]
                .max(pc_limits[1])
                .max(pc_limits[2]),
        }
    }

    fn create_pipeline_cache(&self) -> n::PipelineCache {
        n::PipelineCache {
            modules: FastStorageMap::default(),
        }
    }

    fn destroy_pipeline_cache(&self, _cache: n::PipelineCache) {
        //drop
    }

    fn merge_pipeline_caches<I>(&self, target: &n::PipelineCache, sources: I)
    where
        I: IntoIterator,
        I::Item: Borrow<n::PipelineCache>,
    {
        let mut dst = target.modules.whole_write();
        for source in sources {
            let mut src = source.borrow().modules.whole_write();
            for (key, value) in src.iter() {
                let storage = match dst.entry(key.clone()) {
                    Entry::Vacant(e) => e.insert(FastStorageMap::default()),
                    Entry::Occupied(mut e) => e.into_mut(),
                };
                let mut dst_module = storage.whole_write();
                let mut src_module = value.whole_write();
                for (key_module, value_module) in src_module.iter() {
                    match dst_module.entry(key_module.clone()) {
                        Entry::Vacant(em) => {
                            em.insert(value_module.clone());
                        }
                        Entry::Occupied(em) => {
                            assert_eq!(em.get().library.as_ptr(), value_module.library.as_ptr());
                            assert_eq!(em.get().entry_point_map, value_module.entry_point_map);
                        }
                    }
                }
            }
        }
    }

    fn create_graphics_pipeline<'a>(
        &self,
        pipeline_desc: &pso::GraphicsPipelineDesc<'a, Backend>,
        cache: Option<&n::PipelineCache>,
    ) -> Result<n::GraphicsPipeline, pso::CreationError> {
        debug!("create_graphics_pipeline {:?}", pipeline_desc);
        let pipeline = metal::RenderPipelineDescriptor::new();
        let pipeline_layout = &pipeline_desc.layout;
        let (rp_attachments, subpass) = {
            let pass::Subpass { main_pass, index } = pipeline_desc.subpass;
            (&main_pass.attachments, &main_pass.subpasses[index])
        };

        let (primitive_class, primitive_type) = match pipeline_desc.input_assembler.primitive {
            hal::Primitive::PointList => (MTLPrimitiveTopologyClass::Point, MTLPrimitiveType::Point),
            hal::Primitive::LineList => (MTLPrimitiveTopologyClass::Line, MTLPrimitiveType::Line),
            hal::Primitive::LineStrip => (MTLPrimitiveTopologyClass::Line, MTLPrimitiveType::LineStrip),
            hal::Primitive::TriangleList => (MTLPrimitiveTopologyClass::Triangle, MTLPrimitiveType::Triangle),
            hal::Primitive::TriangleStrip => (MTLPrimitiveTopologyClass::Triangle, MTLPrimitiveType::TriangleStrip),
            _ => (MTLPrimitiveTopologyClass::Unspecified, MTLPrimitiveType::Point) //TODO: double-check
        };
        pipeline.set_input_primitive_topology(primitive_class);

        // Vertex shader
        let (vs_lib, vs_function, _, enable_rasterization) = self.load_shader(
            &pipeline_desc.shaders.vertex,
            pipeline_layout,
            primitive_class,
            cache,
        )?;
        pipeline.set_vertex_function(Some(&vs_function));

        // Fragment shader
        let fs_function;
        let fs_lib = match pipeline_desc.shaders.fragment {
            Some(ref ep) => {
                let (lib, fun, _, _) = self.load_shader(ep, pipeline_layout, primitive_class, cache)?;
                fs_function = fun;
                pipeline.set_fragment_function(Some(&fs_function));
                Some(lib)
            }
            None => {
                // TODO: This is a workaround for what appears to be a Metal validation bug
                // A pixel format is required even though no attachments are provided
                if subpass.colors.is_empty() && subpass.depth_stencil.is_none() {
                    pipeline.set_depth_attachment_pixel_format(metal::MTLPixelFormat::Depth32Float);
                }
                None
            },
        };

        // Other shaders
        if pipeline_desc.shaders.hull.is_some() {
            return Err(pso::CreationError::Shader(ShaderError::UnsupportedStage(pso::Stage::Hull)));
        }
        if pipeline_desc.shaders.domain.is_some() {
            return Err(pso::CreationError::Shader(ShaderError::UnsupportedStage(pso::Stage::Domain)));
        }
        if pipeline_desc.shaders.geometry.is_some() {
            return Err(pso::CreationError::Shader(ShaderError::UnsupportedStage(pso::Stage::Geometry)));
        }

        pipeline.set_rasterization_enabled(enable_rasterization);

        // Assign target formats
        let blend_targets = pipeline_desc.blender.targets
            .iter()
            .chain(iter::repeat(&pso::ColorBlendDesc::EMPTY));
        for (i, (&(mtl_format, _), &pso::ColorBlendDesc(mask, ref blend))) in subpass.target_formats.colors
            .iter()
            .zip(blend_targets)
            .enumerate()
        {
            let desc = pipeline
                .color_attachments()
                .object_at(i)
                .expect("too many color attachments");

            desc.set_pixel_format(mtl_format);
            desc.set_write_mask(conv::map_write_mask(mask));

            if let pso::BlendState::On { ref color, ref alpha } = *blend {
                desc.set_blending_enabled(true);
                let (color_op, color_src, color_dst) = conv::map_blend_op(color);
                let (alpha_op, alpha_src, alpha_dst) = conv::map_blend_op(alpha);

                desc.set_rgb_blend_operation(color_op);
                desc.set_source_rgb_blend_factor(color_src);
                desc.set_destination_rgb_blend_factor(color_dst);

                desc.set_alpha_blend_operation(alpha_op);
                desc.set_source_alpha_blend_factor(alpha_src);
                desc.set_destination_alpha_blend_factor(alpha_dst);
            }
        }
        if let Some(mtl_format) = subpass.target_formats.depth_stencil {
            let orig_format = rp_attachments[subpass.depth_stencil.unwrap().0].format.unwrap();
            if orig_format.is_depth() {
                pipeline.set_depth_attachment_pixel_format(mtl_format);
            }
            if orig_format.is_stencil() {
                pipeline.set_stencil_attachment_pixel_format(mtl_format);
            }
        }

        // Vertex buffers
        let attribute_buffer_index = pipeline_layout.attribute_buffer_index();
        let vertex_descriptor = metal::VertexDescriptor::new();
        let mut vertex_buffers: n::VertexBufferVec = Vec::new();
        trace!("Vertex attribute remapping started");

        for &pso::AttributeDesc { location, binding, element } in &pipeline_desc.attributes {
            let original = pipeline_desc.vertex_buffers
                .iter()
                .find(|vb| vb.binding == binding)
                .expect("no associated vertex buffer found");
            // handle wrapping offsets
            let elem_size = element.format.surface_desc().bits as pso::ElemOffset / 8;
            let (cut_offset, base_offset) = if original.stride == 0 || element.offset + elem_size <= original.stride {
                (element.offset, 0)
            } else {
                let remainder = element.offset % original.stride;
                if remainder + elem_size <= original.stride {
                    (remainder, element.offset - remainder)
                } else {
                    (0, element.offset)
                }
            };
            let relative_index = vertex_buffers
                .iter()
                .position(|(ref vb, offset)| vb.binding == binding && base_offset == *offset)
                .unwrap_or_else(|| {
                    vertex_buffers.push((original.clone(), base_offset));
                    vertex_buffers.len() - 1
                });
            let mtl_buffer_index = attribute_buffer_index as usize + relative_index;
            if mtl_buffer_index >= self.private_caps.max_buffers_per_stage as usize {
                error!("Attribute offset {} exceeds the stride {}, and there is no room for replacement.",
                    element.offset, original.stride);
                return Err(pso::CreationError::Other);
            }
            trace!("\tAttribute[{}] is mapped to vertex buffer[{}] with binding {} and offsets {} + {}",
                location, binding, mtl_buffer_index, base_offset, cut_offset);
            // pass the refined data to Metal
            let mtl_attribute_desc = vertex_descriptor
                .attributes()
                .object_at(location as usize)
                .expect("too many vertex attributes");
            let mtl_vertex_format = conv::map_vertex_format(element.format)
                .expect("unsupported vertex format");
            mtl_attribute_desc.set_format(mtl_vertex_format);
            mtl_attribute_desc.set_buffer_index(mtl_buffer_index as _);
            mtl_attribute_desc.set_offset(cut_offset as _);
        }

        const STRIDE_GRANULARITY: pso::ElemStride = 4; //TODO: work around?
        for (i, (vb, _)) in vertex_buffers.iter().enumerate() {
            let mtl_buffer_desc = vertex_descriptor
                .layouts()
                .object_at(attribute_buffer_index as usize + i)
                .expect("too many vertex descriptor layouts");
            if vb.stride % STRIDE_GRANULARITY != 0 {
                error!("Stride ({}) must be a multiple of {}", vb.stride, STRIDE_GRANULARITY);
                return Err(pso::CreationError::Other);
            }
            if vb.stride != 0 {
                mtl_buffer_desc.set_stride(vb.stride as u64);
                if vb.rate == 0 {
                    mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerVertex);
                } else {
                    mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerInstance);
                    mtl_buffer_desc.set_step_rate(vb.rate as u64);
                }
            } else {
                mtl_buffer_desc.set_stride(256); // big enough to fit all the elements
                mtl_buffer_desc.set_step_function(MTLVertexStepFunction::PerInstance);
                mtl_buffer_desc.set_step_rate(!0);
            }
        }
        if !vertex_buffers.is_empty() {
            pipeline.set_vertex_descriptor(Some(&vertex_descriptor));
        }

        if let pso::PolygonMode::Line(width) = pipeline_desc.rasterizer.polygon_mode {
            validate_line_width(width);
        }

        let rasterizer_state = Some(n::RasterizerState {
            front_winding: conv::map_winding(pipeline_desc.rasterizer.front_face),
            cull_mode: match conv::map_cull_face(pipeline_desc.rasterizer.cull_face) {
                Some(mode) => mode,
                None => {
                    //TODO - Metal validation fails with
                    // RasterizationEnabled is false but the vertex shader's return type is not void
                    error!("Culling both sides is not yet supported");
                    //pipeline.set_rasterization_enabled(false);
                    metal::MTLCullMode::None
                }
            },
            depth_clip: if pipeline_desc.rasterizer.depth_clamping {
                metal::MTLDepthClipMode::Clamp
            } else {
                metal::MTLDepthClipMode::Clip
            },
        });
        let depth_bias = pipeline_desc.rasterizer.depth_bias
            .unwrap_or(pso::State::Static(pso::DepthBias::default()));

        // prepare the depth-stencil state now
        let device = self.shared.device.lock();
        self.shared.service_pipes
            .depth_stencil_states
            .prepare(&pipeline_desc.depth_stencil, &*device);

        device.new_render_pipeline_state(&pipeline)
            .map(|raw|
                n::GraphicsPipeline {
                    vs_lib,
                    fs_lib,
                    raw,
                    primitive_type,
                    attribute_buffer_index,
                    vs_pc_info: pipeline_desc.layout.push_constants.vs,
                    ps_pc_info: pipeline_desc.layout.push_constants.ps,
                    rasterizer_state,
                    depth_bias,
                    depth_stencil_desc: pipeline_desc.depth_stencil.clone(),
                    baked_states: pipeline_desc.baked_states.clone(),
                    vertex_buffers,
                    attachment_formats: subpass.target_formats.clone(),
                })
            .map_err(|err| {
                error!("PSO creation failed: {}", err);
                pso::CreationError::Other
            })
    }

    fn create_compute_pipeline<'a>(
        &self,
        pipeline_desc: &pso::ComputePipelineDesc<'a, Backend>,
        cache: Option<&n::PipelineCache>,
    ) -> Result<n::ComputePipeline, pso::CreationError> {
        debug!("create_compute_pipeline {:?}", pipeline_desc);
        let pipeline = metal::ComputePipelineDescriptor::new();

        let (cs_lib, cs_function, work_group_size, _) = self.load_shader(
            &pipeline_desc.shader,
            &pipeline_desc.layout,
            MTLPrimitiveTopologyClass::Unspecified,
            cache,
        )?;
        pipeline.set_compute_function(Some(&cs_function));

        unsafe {
            self.shared
                .device
                .lock()
                .new_compute_pipeline_state(&pipeline)
        }.map(|raw| n::ComputePipeline {
            cs_lib,
            raw,
            work_group_size,
            pc_info: pipeline_desc.layout.push_constants.cs,
        }).map_err(|err| {
            error!("PSO creation failed: {}", err);
            pso::CreationError::Other
        })
    }

    fn create_framebuffer<I>(
        &self, _render_pass: &n::RenderPass, attachments: I, extent: image::Extent
    ) -> Result<n::Framebuffer, FramebufferError>
    where
        I: IntoIterator,
        I::Item: Borrow<n::ImageView>
    {
        Ok(n::Framebuffer {
            extent,
            attachments: attachments
                .into_iter()
                .map(|at| at.borrow().raw.clone())
                .collect(),
        })
    }

    fn create_shader_module(&self, raw_data: &[u8]) -> Result<n::ShaderModule, ShaderError> {
        //TODO: we can probably at least parse here and save the `Ast`
        let depends_on_pipeline_layout = true; //TODO: !self.private_caps.argument_buffers
        Ok(if depends_on_pipeline_layout {
            n::ShaderModule::Raw(raw_data.to_vec())
        } else {
            let mut options = msl::CompilerOptions::default();
            options.enable_point_size_builtin = false;
            options.resolve_specialized_array_lengths = true;
            options.vertex.invert_y = true;
            let info = Self::compile_shader_library(&self.shared.device, raw_data, &options, self.private_caps.msl_version)?;
            n::ShaderModule::Compiled(info)
        })
    }

    fn create_sampler(&self, info: image::SamplerInfo) -> n::Sampler {
        let descriptor = metal::SamplerDescriptor::new();

        descriptor.set_min_filter(conv::map_filter(info.min_filter));
        descriptor.set_mag_filter(conv::map_filter(info.mag_filter));
        descriptor.set_mip_filter(match info.mip_filter {
            // Note: this shouldn't be required, but Metal appears to be confused when mipmaps
            // are provided even with trivial LOD bias.
            image::Filter::Nearest if info.lod_range.end < image::Lod::from(0.5) => MTLSamplerMipFilter::NotMipmapped,
            image::Filter::Nearest => MTLSamplerMipFilter::Nearest,
            image::Filter::Linear => MTLSamplerMipFilter::Linear,
        });

        if let image::Anisotropic::On(aniso) = info.anisotropic {
            descriptor.set_max_anisotropy(aniso as _);
        }

        let (s, t, r) = info.wrap_mode;
        descriptor.set_address_mode_s(conv::map_wrap_mode(s));
        descriptor.set_address_mode_t(conv::map_wrap_mode(t));
        descriptor.set_address_mode_r(conv::map_wrap_mode(r));

        unsafe { descriptor.set_lod_bias(info.lod_bias.into()) };
        descriptor.set_lod_min_clamp(info.lod_range.start.into());
        descriptor.set_lod_max_clamp(info.lod_range.end.into());
        
        let caps = &self.private_caps;
        // TODO: Clarify minimum macOS version with Apple (43707452)
        if (caps.os_is_mac && caps.has_version_at_least(10, 13)) ||
            (!caps.os_is_mac && caps.has_version_at_least(9, 0)) {
            descriptor.set_lod_average(true); // optimization
        }

        if let Some(fun) = info.comparison {
            descriptor.set_compare_function(conv::map_compare_function(fun));
        }
        if [r, s, t].iter().any(|&am| am == image::WrapMode::Border) {
            descriptor.set_border_color(match info.border.0 {
                0x00000000 => MTLSamplerBorderColor::TransparentBlack,
                0x000000FF => MTLSamplerBorderColor::OpaqueBlack,
                0xFFFFFFFF => MTLSamplerBorderColor::OpaqueWhite,
                other => {
                    error!("Border color 0x{:X} is not supported", other);
                    MTLSamplerBorderColor::TransparentBlack
                }
            });
        }

        n::Sampler(
            self.shared.device
            .lock()
            .new_sampler(&descriptor)
        )
    }

    fn destroy_sampler(&self, _sampler: n::Sampler) {
    }

    fn map_memory<R: RangeArg<u64>>(
        &self, memory: &n::Memory, generic_range: R
    ) -> Result<*mut u8, mapping::Error> {
        let range = memory.resolve(&generic_range);
        debug!("map_memory of size {} at {:?}", memory.size, range);

        let base_ptr = match memory.heap {
            n::MemoryHeap::Public(_, ref cpu_buffer) => cpu_buffer.contents() as *mut u8,
            n::MemoryHeap::Native(_) |
            n::MemoryHeap::Private => panic!("Unable to map memory!"),
        };
        Ok(unsafe { base_ptr.offset(range.start as _) })
    }

    fn unmap_memory(&self, memory: &n::Memory) {
        debug!("unmap_memory of size {}", memory.size);
    }

    fn flush_mapped_memory_ranges<'a, I, R>(&self, iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        debug!("flush_mapped_memory_ranges");
        for item in iter {
            let (memory, ref generic_range) = *item.borrow();
            let range = memory.resolve(generic_range);
            debug!("\trange {:?}", range);

            match memory.heap {
                n::MemoryHeap::Native(_) => unimplemented!(),
                n::MemoryHeap::Public(mt, ref cpu_buffer) if 1<<mt.0 != MemoryTypes::SHARED.bits() as usize => {
                    cpu_buffer.did_modify_range(NSRange {
                        location: range.start as _,
                        length: (range.end - range.start) as _,
                    });
                }
                n::MemoryHeap::Public(..) => continue,
                n::MemoryHeap::Private => panic!("Can't map private memory!"),
            };
        }
    }

    fn invalidate_mapped_memory_ranges<'a, I, R>(&self, iter: I)
    where
        I: IntoIterator,
        I::Item: Borrow<(&'a n::Memory, R)>,
        R: RangeArg<u64>,
    {
        let mut num_syncs = 0;
        debug!("invalidate_mapped_memory_ranges");

        // temporary command buffer to copy the contents from
        // the given buffers into the allocated CPU-visible buffers
        let cmd_queue = self.shared.queue.lock();
        let cmd_buffer = cmd_queue.spawn_temp();
        autoreleasepool(|| {
            let encoder = cmd_buffer.new_blit_command_encoder();

            for item in iter {
                let (memory, ref generic_range) = *item.borrow();
                let range = memory.resolve(generic_range);
                debug!("\trange {:?}", range);

                match memory.heap {
                    n::MemoryHeap::Native(_) => unimplemented!(),
                    n::MemoryHeap::Public(mt, ref cpu_buffer) if 1<<mt.0 != MemoryTypes::SHARED.bits() as usize => {
                        num_syncs += 1;
                        encoder.synchronize_resource(cpu_buffer);
                    }
                    n::MemoryHeap::Public(..) => continue,
                    n::MemoryHeap::Private => panic!("Can't map private memory!"),
                };
            }
            encoder.end_encoding();
        });

        if num_syncs != 0 {
            debug!("\twaiting...");
            cmd_buffer.set_label("invalidate_mapped_memory_ranges");
            cmd_buffer.commit();
            cmd_buffer.wait_until_completed();
        }
    }

    fn create_semaphore(&self) -> n::Semaphore {
        n::Semaphore {
            // Semaphore synchronization between command buffers of the same queue
            // is useless, don't bother even creating one.
            system: if self.private_caps.exposed_queues > 1 {
                Some(n::SystemSemaphore::new())
            } else {
                None
            },
            image_ready: Arc::new(Mutex::new(None)),
        }
    }

    fn create_descriptor_pool<I>(&self, _max_sets: usize, descriptor_ranges: I) -> n::DescriptorPool
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorRangeDesc>,
    {
        let mut counters = n::ResourceData::<n::PoolResourceIndex>::new();

        if self.private_caps.argument_buffers {
            let mut arguments = Vec::new();
            for desc_range in descriptor_ranges {
                let desc = desc_range.borrow();
                let offset_ref = match desc.ty {
                    pso::DescriptorType::Sampler => &mut counters.samplers,
                    pso::DescriptorType::SampledImage => &mut counters.textures,
                    pso::DescriptorType::UniformBuffer | pso::DescriptorType::StorageBuffer => &mut counters.buffers,
                    _ => unimplemented!()
                };
                let index = *offset_ref;
                *offset_ref += desc.count as n::PoolResourceIndex;
                let arg_desc = Self::describe_argument(desc.ty, index as _, desc.count);
                arguments.push(arg_desc);
            }

            let device = self.shared.device.lock();
            let arg_array = metal::Array::from_owned_slice(&arguments);
            let encoder = device.new_argument_encoder(&arg_array);

            let total_size = encoder.encoded_length();
            let raw = device.new_buffer(total_size, MTLResourceOptions::empty());

            n::DescriptorPool::ArgumentBuffer {
                raw,
                range_allocator: RangeAllocator::new(0..total_size),
            }
        } else {
            for desc_range in descriptor_ranges {
                let dr = desc_range.borrow();
                counters.add_many(
                    n::DescriptorContent::from(dr.ty),
                    dr.count as pso::DescriptorBinding,
                );
            }
            n::DescriptorPool::new_emulated(counters)
        }
    }

    fn create_descriptor_set_layout<I, J>(
        &self, binding_iter: I, immutable_samplers: J
    ) -> n::DescriptorSetLayout
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetLayoutBinding>,
        J: IntoIterator,
        J::Item: Borrow<n::Sampler>,
    {
        if self.private_caps.argument_buffers {
            let mut stage_flags = pso::ShaderStageFlags::empty();
            let arguments = binding_iter
                .into_iter()
                .map(|desc| {
                    let desc = desc.borrow();
                    stage_flags |= desc.stage_flags;
                    Self::describe_argument(desc.ty, desc.binding, desc.count)
                })
                .collect::<Vec<_>>();
            let arg_array = metal::Array::from_owned_slice(&arguments);
            let encoder = self.shared.device
                .lock()
                .new_argument_encoder(&arg_array);

            n::DescriptorSetLayout::ArgumentBuffer(encoder, stage_flags)
        } else {
            struct TempSampler {
                sampler: metal::SamplerState,
                binding: pso::DescriptorBinding,
                array_index: pso::DescriptorArrayIndex,
            };
            let mut immutable_sampler_iter = immutable_samplers.into_iter();
            let mut tmp_samplers = Vec::new();
            let mut desc_layouts = Vec::new();

            for set_layout_binding in binding_iter {
                let slb = set_layout_binding.borrow();
                let mut content = native::DescriptorContent::from(slb.ty);
                if slb.immutable_samplers {
                    content |= native::DescriptorContent::IMMUTABLE_SAMPLER;
                    tmp_samplers.extend(immutable_sampler_iter
                        .by_ref()
                        .take(slb.count)
                        .enumerate()
                        .map(|(array_index, sm)| TempSampler {
                            sampler: sm.borrow().0.clone(),
                            binding: slb.binding,
                            array_index,
                        })
                    );
                }
                desc_layouts.extend((0 .. slb.count)
                    .map(|array_index| native::DescriptorLayout {
                        content,
                        stages: slb.stage_flags,
                        binding: slb.binding,
                        array_index,
                    })
                );
            }

            desc_layouts.sort_by_key(|dl| (dl.binding, dl.array_index));
            tmp_samplers.sort_by_key(|ts| (ts.binding, ts.array_index));
            // From here on, we assume that `desc_layouts` has at most a single item for
            // a (binding, array_index) pair. To achieve that, we deduplicate the array now
            desc_layouts.dedup_by(|a, b| {
                if (a.binding, a.array_index) == (b.binding, b.array_index) {
                    debug_assert!(!b.stages.intersects(a.stages));
                    debug_assert_eq!(a.content, b.content); //TODO: double check if this can be demanded
                    b.stages |= a.stages; //`b` is here to stay
                    true
                } else {
                    false
                }
            });

            n::DescriptorSetLayout::Emulated(
                Arc::new(desc_layouts),
                tmp_samplers.into_iter().map(|ts| ts.sampler).collect()
            )
        }
    }

    fn write_descriptor_sets<'a, I, J>(&self, write_iter: I)
    where
        I: IntoIterator<Item = pso::DescriptorSetWrite<'a, Backend, J>>,
        J: IntoIterator,
        J::Item: Borrow<pso::Descriptor<'a, Backend>>,
    {
        debug!("write_descriptor_sets");
        for write in write_iter {
            match *write.set {
                n::DescriptorSet::Emulated { ref pool, ref layouts, ref resources } => {
                    let mut counters = resources.map(|r| r.start);
                    let mut start = None; //TODO: can pre-compute this
                    for (i, layout) in layouts.iter().enumerate() {
                        if layout.binding == write.binding && layout.array_index == write.array_offset {
                            start = Some(i);
                            break;
                        }
                        counters.add(layout.content);
                    }
                    let mut data = pool.write();

                    for (layout, descriptor) in layouts[start.unwrap() ..].iter().zip(write.descriptors) {
                        trace!("\t{:?}", layout);
                        match *descriptor.borrow() {
                            pso::Descriptor::Sampler(sam) => {
                                debug_assert!(!layout.content.contains(n::DescriptorContent::IMMUTABLE_SAMPLER));
                                data.samplers[counters.samplers as usize] = Some(AsNative::from(sam.0.as_ref()));
                            }
                            pso::Descriptor::Image(tex, il) => {
                                data.textures[counters.textures as usize] = Some((AsNative::from(tex.raw.as_ref()), il));
                            }
                            pso::Descriptor::CombinedImageSampler(tex, il, sam) => {
                                if !layout.content.contains(n::DescriptorContent::IMMUTABLE_SAMPLER) {
                                    data.samplers[counters.samplers as usize] = Some(AsNative::from(sam.0.as_ref()));
                                }
                                data.textures[counters.textures as usize] = Some((AsNative::from(tex.raw.as_ref()), il));
                            }
                            pso::Descriptor::UniformTexelBuffer(view) |
                            pso::Descriptor::StorageTexelBuffer(view) => {
                                data.textures[counters.textures as usize] = Some((AsNative::from(view.raw.as_ref()), image::Layout::General));
                            }
                            pso::Descriptor::Buffer(buf, ref range) => {
                                let start = buf.range.start + range.start.unwrap_or(0);
                                if let Some(end) = range.end {
                                    debug_assert!(buf.range.start + end <= buf.range.end);
                                };
                                data.buffers[counters.buffers as usize] = Some((AsNative::from(buf.raw.as_ref()), start));
                            }
                        }
                        counters.add(layout.content);
                    }
                }
                n::DescriptorSet::ArgumentBuffer { ref raw, offset, ref encoder, .. } => {
                    debug_assert!(self.private_caps.argument_buffers);

                    encoder.set_argument_buffer(raw, offset);
                    //TODO: range checks, need to keep some layout metadata around
                    assert_eq!(write.array_offset, 0); //TODO

                    for descriptor in write.descriptors {
                        match *descriptor.borrow() {
                            pso::Descriptor::Sampler(sampler) => {
                                encoder.set_sampler_states(&[&sampler.0], write.binding as _);
                            }
                            pso::Descriptor::Image(image, _layout) => {
                                encoder.set_textures(&[&image.raw], write.binding as _);
                            }
                            pso::Descriptor::Buffer(buffer, ref range) => {
                                encoder.set_buffer(&buffer.raw, buffer.range.start + range.start.unwrap_or(0), write.binding as _);
                            }
                            pso::Descriptor::CombinedImageSampler(..) |
                            pso::Descriptor::UniformTexelBuffer(..) |
                            pso::Descriptor::StorageTexelBuffer(..) => unimplemented!(),
                        }
                    }
                }
            }
        }
    }

    fn copy_descriptor_sets<'a, I>(&self, copies: I)
    where
        I: IntoIterator,
        I::Item: Borrow<pso::DescriptorSetCopy<'a, Backend>>,
    {
        for _copy in copies {
            unimplemented!()
        }
    }

    fn destroy_descriptor_pool(&self, _pool: n::DescriptorPool) {
    }

    fn destroy_descriptor_set_layout(&self, _layout: n::DescriptorSetLayout) {
    }

    fn destroy_pipeline_layout(&self, _pipeline_layout: n::PipelineLayout) {
    }

    fn destroy_shader_module(&self, _module: n::ShaderModule) {
    }

    fn destroy_render_pass(&self, _pass: n::RenderPass) {
    }

    fn destroy_graphics_pipeline(&self, _pipeline: n::GraphicsPipeline) {
    }

    fn destroy_compute_pipeline(&self, _pipeline: n::ComputePipeline) {
    }

    fn destroy_framebuffer(&self, _buffer: n::Framebuffer) {
    }

    fn destroy_semaphore(&self, _semaphore: n::Semaphore) {
    }

    fn allocate_memory(&self, memory_type: hal::MemoryTypeId, size: u64) -> Result<n::Memory, OutOfMemory> {
        let (storage, cache) = MemoryTypes::describe(memory_type.0);
        let device = self.shared.device.lock();
        debug!("allocate_memory type {:?} of size {}", memory_type, size);

        // Heaps cannot be used for CPU coherent resources
        //TEMP: MacOS supports Private only, iOS and tvOS can do private/shared
        let heap = if self.private_caps.resource_heaps && storage != MTLStorageMode::Shared && false {
            let descriptor = metal::HeapDescriptor::new();
            descriptor.set_storage_mode(storage);
            descriptor.set_cpu_cache_mode(cache);
            descriptor.set_size(size);
            let heap_raw = device.new_heap(&descriptor);
            n::MemoryHeap::Native(heap_raw)
        } else if storage == MTLStorageMode::Private {
            n::MemoryHeap::Private
        } else {
            let options = conv::resource_options_from_storage_and_cache(storage, cache);
            let cpu_buffer = device.new_buffer(size, options);
            debug!("\tbacked by cpu buffer {:?}", cpu_buffer.as_ptr());
            n::MemoryHeap::Public(memory_type, cpu_buffer)
        };

        Ok(n::Memory::new(heap, size))
    }

    fn free_memory(&self, memory: n::Memory) {
        debug!("free_memory of size {}", memory.size);
        if let n::MemoryHeap::Public(_, ref cpu_buffer) = memory.heap {
            debug!("\tbacked by cpu buffer {:?}", cpu_buffer.as_ptr());
        }
    }

    fn create_buffer(
        &self, size: u64, usage: buffer::Usage
    ) -> Result<n::UnboundBuffer, buffer::CreationError> {
        debug!("create_buffer of size {} and usage {:?}", size, usage);
        Ok(n::UnboundBuffer {
            size,
            usage,
        })
    }

    fn get_buffer_requirements(&self, buffer: &n::UnboundBuffer) -> memory::Requirements {
        let mut max_size = buffer.size;
        let mut max_alignment = self.private_caps.buffer_alignment;

        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the buffer with, so we test them
            // all get the most stringent ones.
            for (i, _mt) in self.memory_types.iter().enumerate() {
                let (storage, cache) = MemoryTypes::describe(i);
                let options = conv::resource_options_from_storage_and_cache(storage, cache);
                let requirements = self.shared.device
                    .lock()
                    .heap_buffer_size_and_align(buffer.size, options);
                max_size = cmp::max(max_size, requirements.size);
                max_alignment = cmp::max(max_alignment, requirements.align);
            }
        }

        // based on Metal validation error for view creation:
        // failed assertion `BytesPerRow of a buffer-backed texture with pixelFormat(XXX) must be aligned to 256 bytes
        const SIZE_MASK: u64 = 0xFF;
        let supports_texel_view = buffer.usage.intersects(
            buffer::Usage::UNIFORM_TEXEL |
            buffer::Usage::STORAGE_TEXEL
        );

        memory::Requirements {
            size: (max_size + SIZE_MASK) & !SIZE_MASK,
            alignment: max_alignment,
            type_mask: if !supports_texel_view || self.private_caps.shared_textures {
                MemoryTypes::all().bits()
            } else {
                (MemoryTypes::all() ^ MemoryTypes::SHARED).bits()
            },
        }
    }

    fn bind_buffer_memory(
        &self, memory: &n::Memory, offset: u64, buffer: n::UnboundBuffer
    ) -> Result<n::Buffer, BindError> {
        debug!("bind_buffer_memory of size {} at offset {}", buffer.size, offset);
        let (raw, options, range) = match memory.heap {
            n::MemoryHeap::Native(ref heap) => {
                let resource_options = conv::resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode(),
                );
                let raw = heap.new_buffer(buffer.size, resource_options)
                    .unwrap_or_else(|| {
                        // TODO: disable hazard tracking?
                        self.shared.device
                            .lock()
                            .new_buffer(buffer.size, resource_options)
                    });
                (raw, resource_options, 0 .. buffer.size) //TODO?
            }
            n::MemoryHeap::Public(mt, ref cpu_buffer) => {
                debug!("\tmapped to public heap with address {:?}", cpu_buffer.as_ptr());
                let (storage, cache) = MemoryTypes::describe(mt.0);
                let options = conv::resource_options_from_storage_and_cache(storage, cache);
                (cpu_buffer.clone(), options, offset .. offset + buffer.size)
            }
            n::MemoryHeap::Private => {
                //TODO: check for aliasing
                let options = MTLResourceOptions::StorageModePrivate |
                    MTLResourceOptions::CPUCacheModeDefaultCache;
                let raw = self.shared.device
                    .lock()
                    .new_buffer(buffer.size, options);
                (raw, options, 0 .. buffer.size)
            }
        };

        Ok(n::Buffer {
            raw,
            range,
            options,
        })
    }

    fn destroy_buffer(&self, buffer: n::Buffer) {
        debug!("destroy_buffer {:?} occupying memory {:?}", buffer.raw.as_ptr(), buffer.range);
    }

    fn create_buffer_view<R: RangeArg<u64>>(
        &self, buffer: &n::Buffer, format_maybe: Option<format::Format>, range: R
    ) -> Result<n::BufferView, buffer::ViewCreationError> {
        let start = buffer.range.start + *range.start().unwrap_or(&0);
        let end_rough = match range.end() {
            Some(end) => buffer.range.start + end,
            None => buffer.range.end,
        };
        let format = match format_maybe {
            Some(fmt) => fmt,
            None => return Err(buffer::ViewCreationError::UnsupportedFormat { format: format_maybe }),
        };
        let format_desc = format.surface_desc();
        if format_desc.aspects != format::Aspects::COLOR || format_desc.is_compressed() {
            // Vadlidator says "Linear texture: cannot create compressed, depth, or stencil textures"
            return Err(buffer::ViewCreationError::UnsupportedFormat { format: format_maybe })
        }

        //Note: we rely on SPIRV-Cross to use the proper 2D texel indexing here
        let texel_count = (end_rough - start) * 8 / format_desc.bits as u64;
        let col_count = cmp::min(texel_count, self.private_caps.max_texture_size);
        let row_count = (texel_count + self.private_caps.max_texture_size - 1) / self.private_caps.max_texture_size;
        let mtl_format = self.private_caps
            .map_format(format)
            .ok_or(buffer::ViewCreationError::UnsupportedFormat { format: format_maybe })?;

        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_texture_type(MTLTextureType::D2);
        descriptor.set_width(col_count);
        descriptor.set_height(row_count);
        descriptor.set_mipmap_level_count(1);
        descriptor.set_pixel_format(mtl_format);
        descriptor.set_resource_options(buffer.options);
        descriptor.set_storage_mode(buffer.raw.storage_mode());
        descriptor.set_usage(metal::MTLTextureUsage::ShaderRead);

        let align_mask = self.private_caps.buffer_alignment - 1;
        let stride = (col_count * (format_desc.bits as u64 / 8) + align_mask) & !align_mask;

        Ok(n::BufferView {
            raw: buffer.raw.new_texture_from_contents(&descriptor, start, stride),
        })
    }

    fn destroy_buffer_view(&self, _view: n::BufferView) {
        //nothing to do
    }

    fn create_image(
        &self,
        kind: image::Kind,
        mip_levels: image::Level,
        format: format::Format,
        tiling: image::Tiling,
        usage: image::Usage,
        view_caps: image::ViewCapabilities,
    ) -> Result<n::UnboundImage, image::CreationError> {
        debug!("create_image {:?} with {} mips of {:?} {:?} and usage {:?}",
            kind, mip_levels, format, tiling, usage);

        let is_cube = view_caps.contains(image::ViewCapabilities::KIND_CUBE);
        let mtl_format = self.private_caps
            .map_format(format)
            .ok_or(image::CreationError::Format(format))?;

        let descriptor = metal::TextureDescriptor::new();

        let (mtl_type, num_layers) = match kind {
            image::Kind::D1(_, 1) => {
                assert!(!is_cube);
                (MTLTextureType::D1, None)
            }
            image::Kind::D1(_, layers) => {
                assert!(!is_cube);
                (MTLTextureType::D1Array, Some(layers))
            }
            image::Kind::D2(_, _, layers, 1) => {
                if is_cube && layers > 6 {
                    assert_eq!(layers % 6, 0);
                    (MTLTextureType::CubeArray, Some(layers / 6))
                } else if is_cube {
                    assert_eq!(layers, 6);
                    (MTLTextureType::Cube, None)
                } else if layers > 1 {
                    (MTLTextureType::D2Array, Some(layers))
                } else {
                    (MTLTextureType::D2, None)
                }
            }
            image::Kind::D2(_, _, 1, samples) if !is_cube => {
                descriptor.set_sample_count(samples as u64);
                (MTLTextureType::D2Multisample, None)
            }
            image::Kind::D2(..) => {
                error!("Multi-sampled array textures or cubes are not supported: {:?}", kind);
                return Err(image::CreationError::Kind)
            }
            image::Kind::D3(..) => {
                assert!(!is_cube && !view_caps.contains(image::ViewCapabilities::KIND_2D_ARRAY));
                (MTLTextureType::D3, None)
            }
        };

        descriptor.set_texture_type(mtl_type);
        if let Some(count) = num_layers {
            descriptor.set_array_length(count as u64);
        }
        let extent = kind.extent();
        descriptor.set_width(extent.width as u64);
        descriptor.set_height(extent.height as u64);
        descriptor.set_depth(extent.depth as u64);
        descriptor.set_mipmap_level_count(mip_levels as u64);
        descriptor.set_pixel_format(mtl_format);
        descriptor.set_usage(conv::map_texture_usage(usage, tiling));

        let format_desc = format.surface_desc();
        let mip_sizes = (0 .. mip_levels)
            .map(|level| {
                let pitches = n::Image::pitches_impl(extent.at_level(level), format_desc);
                num_layers.unwrap_or(1) as buffer::Offset * pitches[3]
            })
            .collect();

        let host_usage = image::Usage::TRANSFER_SRC | image::Usage::TRANSFER_DST;
        let host_visible = mtl_type == MTLTextureType::D2 &&
            mip_levels == 1 && num_layers.is_none() &&
            format_desc.aspects.contains(format::Aspects::COLOR) &&
            tiling == image::Tiling::Linear &&
            host_usage.contains(usage);

        Ok(n::UnboundImage {
            texture_desc: descriptor,
            format,
            kind,
            mip_sizes,
            host_visible,
        })
    }

    fn get_image_requirements(&self, image: &n::UnboundImage) -> memory::Requirements {
        if self.private_caps.resource_heaps {
            // We don't know what memory type the user will try to allocate the image with, so we test them
            // all get the most stringent ones. Note we don't check Shared because heaps can't use it
            let mut max_size = 0;
            let mut max_alignment = 0;
            let types = if image.host_visible {
                MemoryTypes::all()
            } else {
                MemoryTypes::PRIVATE
            };
            for (i, _) in self.memory_types.iter().enumerate() {
                if !types.contains(MemoryTypes::from_bits(1 << i).unwrap()) {
                    continue
                }
                let (storage, cache_mode) = MemoryTypes::describe(i);
                image.texture_desc.set_storage_mode(storage);
                image.texture_desc.set_cpu_cache_mode(cache_mode);

                let requirements = self.shared.device
                    .lock()
                    .heap_texture_size_and_align(&image.texture_desc);
                max_size = cmp::max(max_size, requirements.size);
                max_alignment = cmp::max(max_alignment, requirements.align);
            }
            memory::Requirements {
                size: max_size,
                alignment: max_alignment,
                type_mask: types.bits(),
            }
        } else if image.host_visible {
            assert_eq!(image.mip_sizes.len(), 1);
            let mask = self.private_caps.buffer_alignment - 1;
            memory::Requirements {
                size: (image.mip_sizes[0] + mask) & !mask,
                alignment: self.private_caps.buffer_alignment,
                type_mask: MemoryTypes::all().bits(),
            }
        } else {
            memory::Requirements {
                size: image.mip_sizes.iter().sum(),
                alignment: 4,
                type_mask: MemoryTypes::PRIVATE.bits(),
            }
        }
    }

    fn get_image_subresource_footprint(
        &self, image: &n::Image, sub: image::Subresource
    ) -> image::SubresourceFootprint {
        let num_layers = image.kind.num_layers() as buffer::Offset;
        let level_offset = (0 .. sub.level).fold(0, |offset, level| {
            let pitches = image.pitches(level);
            offset + num_layers * pitches[3]
        });
        let pitches = image.pitches(sub.level);
        let layer_offset = level_offset + sub.layer as buffer::Offset * pitches[3];
        image::SubresourceFootprint {
            slice: layer_offset .. layer_offset + pitches[3],
            row_pitch: pitches[1] as _,
            depth_pitch: pitches[2] as _,
            array_pitch: pitches[3] as _,
        }
    }

    fn bind_image_memory(
        &self, memory: &n::Memory, offset: u64, image: n::UnboundImage
    ) -> Result<n::Image, BindError> {
        let base = image.format.base_format();
        let format_desc = base.0.desc();

        let like = match memory.heap {
            n::MemoryHeap::Native(ref heap) => {
                let resource_options = conv::resource_options_from_storage_and_cache(
                    heap.storage_mode(),
                    heap.cpu_cache_mode());
                image.texture_desc.set_resource_options(resource_options);
                n::ImageLike::Texture(
                    heap.new_texture(&image.texture_desc)
                        .unwrap_or_else(|| {
                            // TODO: disable hazard tracking?
                            self.shared.device
                                .lock()
                                .new_texture(&image.texture_desc)
                        })
                )
            },
            n::MemoryHeap::Public(_memory_type, ref cpu_buffer) => {
                assert_eq!(image.mip_sizes.len(), 1);
                n::ImageLike::Buffer(n::Buffer {
                    raw: cpu_buffer.clone(),
                    range: offset .. offset + image.mip_sizes[0] as u64,
                    options: MTLResourceOptions::StorageModeShared,
                })
            }
            n::MemoryHeap::Private => {
                image.texture_desc.set_storage_mode(MTLStorageMode::Private);
                n::ImageLike::Texture(
                    self.shared.device
                        .lock()
                        .new_texture(&image.texture_desc)
                )
            }
        };

        Ok(n::Image {
            like,
            kind: image.kind,
            format_desc,
            shader_channel: base.1.into(),
            mtl_format: match self.private_caps.map_format(image.format) {
                Some(format) => format,
                None => {
                    error!("failed to find corresponding Metal format for {:?}", image.format);
                    return Err(BindError::OutOfBounds);
                },
            },
            mtl_type: image.texture_desc.texture_type(),
        })
    }

    fn destroy_image(&self, _image: n::Image) {
    }

    fn create_image_view(
        &self,
        image: &n::Image,
        kind: image::ViewKind,
        format: format::Format,
        swizzle: format::Swizzle,
        range: image::SubresourceRange,
    ) -> Result<n::ImageView, image::ViewError> {
        let mtl_format = match self.private_caps.map_format_with_swizzle(format, swizzle) {
            Some(f) => f,
            None => {
                error!("failed to swizzle format {:?} with {:?}", format, swizzle);
                return Err(image::ViewError::BadFormat);
            },
        };
        let raw = image.like.as_texture();
        let full_range = image::SubresourceRange {
            aspects: image.format_desc.aspects,
            levels: 0 .. raw.mipmap_level_count() as image::Level,
            layers: 0 .. image.kind.num_layers(),
        };
        let mtl_type = conv::map_texture_type(kind);

        let view = if
            mtl_format == image.mtl_format &&
            mtl_type == image.mtl_type &&
            swizzle == format::Swizzle::NO &&
            range == full_range
        {
            // Some images are marked as framebuffer-only, and we can't create aliases of them.
            // Also helps working around Metal bugs with aliased array textures.
            raw.to_owned()
        } else {
            raw.new_texture_view_from_slice(
                mtl_format,
                mtl_type,
                NSRange {
                    location: range.levels.start as _,
                    length: (range.levels.end - range.levels.start) as _,
                },
                NSRange {
                    location: range.layers.start as _,
                    length: (range.layers.end - range.layers.start) as _,
                },
            )
        };

        Ok(n::ImageView { raw: view, mtl_format })
    }

    fn destroy_image_view(&self, _view: n::ImageView) {
    }

    fn create_fence(&self, signaled: bool) -> n::Fence {
        n::Fence(RefCell::new(n::FenceInner::Idle { signaled }))
    }
    fn reset_fence(&self, fence: &n::Fence) {
        *fence.0.borrow_mut() = n::FenceInner::Idle { signaled: false };
    }
    fn wait_for_fence(&self, fence: &n::Fence, timeout_ns: u64) -> bool {
        fn to_ns(duration: time::Duration) -> u64 {
            duration.as_secs() * 1_000_000_000 + duration.subsec_nanos() as u64
        }

        debug!("wait_for_fence {:?} for {} ms", fence, timeout_ns);
        let inner = fence.0.borrow();
        let cmd_buf = match *inner {
            native::FenceInner::Idle { signaled } => return signaled,
            native::FenceInner::Pending(ref cmd_buf) => cmd_buf,
        };
        if timeout_ns == !0 {
            cmd_buf.wait_until_completed();
            return true
        }

        let start = time::Instant::now();
        loop {
            if let metal::MTLCommandBufferStatus::Completed = cmd_buf.status() {
                return true
            }
            if to_ns(start.elapsed()) >= timeout_ns {
                return false;
            }
            thread::sleep(time::Duration::from_millis(1));
        }
    }
    fn get_fence_status(&self, fence: &n::Fence) -> bool {
        match *fence.0.borrow() {
            native::FenceInner::Idle { signaled } => signaled,
            native::FenceInner::Pending(ref cmd_buf) => match cmd_buf.status() {
                metal::MTLCommandBufferStatus::Completed => true,
                _ => false,
            },
        }
    }
    fn destroy_fence(&self, _fence: n::Fence) {
    }

    fn create_query_pool(
        &self, ty: query::Type, count: query::Id
    ) -> Result<n::QueryPool, query::Error> {
        match ty {
            query::Type::Occlusion => {
                let range = self.shared.visibility.allocator
                    .lock()
                    .allocate_range(count)
                    .map_err(|_| {
                        error!("Not enough space to allocate an occlusion query pool");
                    })?;
                Ok(n::QueryPool::Occlusion(range))
            }
            _ => {
                error!("Only occlusion queries are currently supported");
                Err(())
            }
        }
    }

    fn destroy_query_pool(&self, pool: n::QueryPool) {
        match pool {
            n::QueryPool::Occlusion(range) => {
                self.shared.visibility.allocator
                    .lock()
                    .free_range(range);
            }
        }
    }

    fn get_query_pool_results(
        &self, pool: &n::QueryPool, queries: Range<query::Id>,
        data: &mut [u8], stride: buffer::Offset,
        flags: query::ResultFlags,
    ) -> Result<bool, query::Error> {
        let is_ready = match *pool {
            native::QueryPool::Occlusion(ref pool_range) => {
                let visibility = &self.shared.visibility;
                let is_ready = if flags.contains(query::ResultFlags::WAIT) {
                    let mut guard = visibility.allocator.lock();
                    while !visibility.are_available(pool_range.start, &queries) {
                        visibility.condvar.wait(&mut guard);
                    }
                    true
                } else {
                    visibility.are_available(pool_range.start, &queries)
                };

                let size_data = mem::size_of::<u64>() as buffer::Offset;
                if stride == size_data && flags.contains(query::ResultFlags::BITS_64) &&
                    !flags.contains(query::ResultFlags::WITH_AVAILABILITY)
                {
                    // if stride is matching, copy everything in one go
                    unsafe {
                        ptr::copy_nonoverlapping(
                            (visibility.buffer.contents() as *const u8)
                                .offset((pool_range.start + queries.start) as isize * size_data as isize),
                            data.as_mut_ptr(),
                            stride as usize * (queries.end - queries.start) as usize,
                        )
                    };
                } else {
                    // copy parts of individual entries
                    for i in 0 .. queries.end - queries.start {
                        let absolute_index = (pool_range.start + queries.start + i) as isize;
                        let value = unsafe {
                            *(visibility.buffer.contents() as *const u64).offset(absolute_index)
                        };
                        let availability = unsafe {
                            let base = (visibility.buffer.contents() as *const u8)
                                .offset(visibility.availability_offset as isize);
                            *(base as *const u32).offset(absolute_index)
                        };
                        let data_ptr = data[i as usize * stride as usize ..].as_mut_ptr();
                        unsafe {
                            if flags.contains(query::ResultFlags::BITS_64) {
                                *(data_ptr as *mut u64) = value;
                                if flags.contains(query::ResultFlags::WITH_AVAILABILITY) {
                                    *(data_ptr as *mut u64).offset(1) = availability as u64;
                                }
                            } else {
                                *(data_ptr as *mut u32) = value as u32;
                                if flags.contains(query::ResultFlags::WITH_AVAILABILITY) {
                                    *(data_ptr as *mut u32).offset(1) = availability;
                                }
                            }
                        }
                    }
                }

                is_ready
            }
        };

        Ok(is_ready)
    }

    fn create_swapchain(
        &self,
        surface: &mut Surface,
        config: hal::SwapchainConfig,
        old_swapchain: Option<Swapchain>,
    ) -> (Swapchain, hal::Backbuffer<Backend>) {
        self.build_swapchain(surface, config, old_swapchain)
    }

    fn destroy_swapchain(&self, _swapchain: Swapchain) {
    }

    fn wait_idle(&self) -> Result<(), error::HostExecutionError> {
        command::QueueInner::wait_idle(&self.shared.queue);
        Ok(())
    }
}

#[test]
fn test_send_sync() {
    fn foo<T: Send+Sync>() {}
    foo::<Device>()
}
