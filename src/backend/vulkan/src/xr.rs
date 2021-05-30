use ash::vk::{self, Handle};
use hal::image;
use std::{
    ffi::{CStr, CString},
    sync::{atomic::Ordering, Mutex},
};

#[cfg(feature = "use-openxr")]
use once_cell::sync::Lazy;
#[cfg(feature = "use-openxr")]
use std::collections::HashMap;

use std::sync::atomic::AtomicBool;

use crate::{conv, native};

#[cfg(feature = "use-openxr")]
pub(crate) struct Instance {
    instance: openxr::Instance,
    system: openxr::SystemId,
    required_device_extension_properties: Vec<CString>,
    required_instance_extension_properties: Vec<CString>,
    requirements: openxr::vulkan::Requirements,

    session: Option<openxr::Session<openxr::Vulkan>>,
    frame_waiter: Option<openxr::FrameWaiter>,
    frame_stream: Option<openxr::FrameStream<openxr::Vulkan>>,
    space: Option<openxr::Space>,
    // FIXME add field for all -validations state, that'd track separate parts

    // swapchain raw images
    raw_images: HashMap<u64, bool>,
}

#[cfg(feature = "use-openxr")]
impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenXR[...]")
    }
}

#[cfg(feature = "use-openxr")]
pub(crate) static INSTANCE: Lazy<Mutex<Option<Instance>>> = Lazy::new(|| Mutex::new(None));
static INSTANCE_SET: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
#[cfg(feature = "use-openxr")]
pub struct OpenXR {}

#[cfg(feature = "use-openxr")]
#[derive(Debug)]
pub enum Error {
    Unknown,
}

#[cfg(feature = "use-openxr")]
impl OpenXR {
    pub fn configure(instance: openxr::Instance) -> Result<OpenXR, Error> {
        let instance_props = instance.properties().unwrap();
        println!(
            "OpenXR instance: runtime={:?}, version={}.{}.{}",
            instance_props.runtime_name,
            instance_props.runtime_version.major(),
            instance_props.runtime_version.minor(),
            instance_props.runtime_version.patch()
        );

        let system = instance
            .system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY)
            .unwrap();

        let requirements = instance
            .graphics_requirements::<openxr::Vulkan>(system)
            .unwrap();

        INSTANCE_SET.store(true, Ordering::SeqCst);

        *INSTANCE.lock().unwrap() = Some(Instance {
            instance,
            system,
            required_device_extension_properties: Vec::new(),
            required_instance_extension_properties: Vec::new(),
            requirements,
            space: None,
            session: None,
            frame_waiter: None,
            frame_stream: None,
            raw_images: HashMap::new(),
        });

        Ok(OpenXR {})
    }

    pub fn get_session_handles() -> (
        openxr::Session<openxr::Vulkan>,
        openxr::FrameWaiter,
        openxr::FrameStream<openxr::Vulkan>,
        openxr::Space,
        openxr::SystemId,
    ) {
        let mut instance_guard = INSTANCE.lock().unwrap();
        let openxr_instance = instance_guard.as_mut().unwrap();

        (
            openxr_instance.session.take().unwrap().clone(),
            openxr_instance.frame_waiter.take().unwrap(),
            openxr_instance.frame_stream.take().unwrap(),
            openxr_instance.space.take().unwrap(),
            openxr_instance.system,
        )
    }

    pub fn texture_from_raw_image(
        raw_image: u64,
        kind: image::Kind,
        view_caps: image::ViewCapabilities,
    ) -> Result<native::Image, image::CreationError> {
        let vk_image = vk::Image::from_raw(raw_image);

        let flags = conv::map_view_capabilities(view_caps);
        let image_type = match kind {
            image::Kind::D1(..) => vk::ImageType::TYPE_1D,
            image::Kind::D2(..) => vk::ImageType::TYPE_2D,
            image::Kind::D3(..) => vk::ImageType::TYPE_3D,
        };

        let image = native::Image {
            raw: vk_image,
            ty: image_type,
            flags,
            extent: conv::map_extent(kind.extent()),
        };

        Ok(image)
    }
}

#[inline]
pub(crate) fn in_use() -> bool {
    #[cfg(feature = "use-openxr")]
    {
        INSTANCE_SET.load(Ordering::SeqCst)
    }

    #[cfg(not(feature = "use-openxr"))]
    false
}

#[cfg(feature = "use-openxr")]
impl Instance {
    pub(crate) fn get_device(&mut self, vk_instance: vk::Instance) -> vk::PhysicalDevice {
        let vk_physical_device = vk::PhysicalDevice::from_raw(
            self.instance
                .vulkan_graphics_device(self.system, vk_instance.as_raw() as _)
                .unwrap() as _,
        );

        self.required_device_extension_properties = self
            .instance
            .vulkan_device_extensions(self.system)
            .unwrap()
            .split(' ')
            .map(|x| CString::new(x).unwrap())
            .collect::<Vec<_>>();

        self.required_instance_extension_properties = self
            .instance
            .vulkan_instance_extensions(self.system)
            .unwrap()
            .split(' ')
            .map(|x| CString::new(x).unwrap())
            .collect::<Vec<_>>();

        vk_physical_device
    }

    pub(crate) fn verify_vulkan_version(&self, entry: &ash::Entry) {
        let vk_version = entry
            .try_enumerate_instance_version()
            .unwrap()
            .unwrap_or_else(|| vk::make_version(1, 0, 0));

        let vk_version = openxr::Version::new(
            vk::version_major(vk_version) as u16,
            vk::version_major(vk_version) as u16,
            0,
        );

        if self.requirements.min_api_version_supported > vk_version {
            panic!(
                "OpenXR runtime requires Vulkan version > {}",
                self.requirements.min_api_version_supported
            );
        }
    }

    pub(crate) fn verify_instance_extensions(&self, extensions: &[vk::ExtensionProperties]) {
        for ext in &self.required_instance_extension_properties {
            unsafe {
                if !extensions.iter().any(|inst_ext| {
                    CStr::from_ptr(inst_ext.extension_name.as_ptr()) == ext.as_c_str()
                }) {
                    panic!(
                        "OpenXR runtime requires missing Vulkan instance extension {:?}",
                        ext
                    );
                }
            }
        }
    }

    pub(crate) fn add_required_instance_extensions(&self, extensions: &mut Vec<&CStr>) {
        let mut failed_extensions = Vec::new();

        for ext in &self.required_instance_extension_properties {
            if !extensions.iter().any(|&e| e == ext.as_c_str()) {
                match OpenXRExtension::from_c_string(ext) {
                    Ok(ext) => {
                        println!("Adding an instance extension {:?}", ext);
                        extensions.push(ext.to_c_str())
                    }
                    Err(_) => failed_extensions.push(ext.as_c_str()),
                }
            }
        }

        if failed_extensions.len() > 0 {
            for extension in failed_extensions {
                println!(
                    "(Xrbevy) doesn't recognize a required extension: {:?}",
                    extension
                );
            }

            panic!("Unknown xrbevy instance extensions detected. Please submit an issue");
        }
    }

    pub(crate) fn add_required_device_extensions(&self, extensions: &mut Vec<&CStr>) {
        let mut failed_extensions = Vec::new();

        for ext in &self.required_device_extension_properties {
            if !extensions.iter().any(|&e| e == ext.as_c_str()) {
                match OpenXRExtension::from_c_string(ext) {
                    Ok(ext) => {
                        println!("Adding a device extension {:?}", ext);
                        extensions.push(ext.to_c_str())
                    }
                    Err(_) => failed_extensions.push(ext.as_c_str()),
                }
            }
        }

        if failed_extensions.len() > 0 {
            for extension in failed_extensions {
                println!(
                    "(Xrbevy) doesn't recognize a required extension: {:?}",
                    extension
                );
            }

            panic!("Unknown xrbevy device extensions detected. Please submit an issue");
        }
    }

    pub(crate) fn create_session(
        &mut self,
        vk_instance: vk::Instance,
        vk_physical_device: vk::PhysicalDevice,
        vk_device: vk::Device,
        queue_family_index: u32,
    ) {
        let (xr_session, xr_frame_wait, xr_frame_stream) = unsafe {
            self.instance.create_session::<openxr::Vulkan>(
                self.system,
                &openxr::vulkan::SessionCreateInfo {
                    instance: vk_instance.as_raw() as _,
                    physical_device: vk_physical_device.as_raw() as _,
                    device: vk_device.as_raw() as _,
                    queue_family_index: queue_family_index as _,
                    queue_index: 0,
                },
            )
        }
        .unwrap();

        let stage = xr_session
            .create_reference_space(openxr::ReferenceSpaceType::STAGE, openxr::Posef::IDENTITY)
            .unwrap();

        self.session = Some(xr_session);
        self.frame_waiter = Some(xr_frame_wait);
        self.frame_stream = Some(xr_frame_stream);
        self.space = Some(stage);

        println!("Session created!");
    }

    pub(crate) fn add_raw_image(&mut self, raw_image_id: u64) {
        self.raw_images.insert(raw_image_id, true);
    }

    pub(crate) fn contains_raw_image(&self, raw_image_id: u64) -> bool {
        self.raw_images.contains_key(&raw_image_id)
    }
}

macro_rules! cstr {
    ($s:literal ) => {{
        unsafe { std::mem::transmute::<_, &std::ffi::CStr>(concat!($s, "\0")) }
    }};
}

macro_rules! openxr_extensions {
    ($(($extension: ident, $string: tt, $idx: tt),)*) => {
        $(
            const $extension: &CStr = cstr!($string);
        )*

        bitflags::bitflags! {
            pub struct OpenXRExtension: u64 {
                $(const $extension = $idx;)+
            }
        }

        impl OpenXRExtension {
            pub(crate) fn to_c_str(self) -> &'static CStr {
                match self {
                    $(
                        OpenXRExtension::$extension => $extension,
                    )+
                    _ => panic!("unknown OpenXR extension requested")
                }
            }

            pub(crate) fn from_c_string(str: &CString) -> Result<Self, Error> {
                let str = str.to_str().unwrap(); // FIXME add error checking

                let res = match str {
                    $($string => OpenXRExtension::$extension,)+
                    _ => return Err(Error::Unknown),
                };

                Ok(res)
            }
        }
    }
}

// a list of possible extensions - last number is just ITEM ^2, can keep increasing
#[cfg(feature = "use-openxr")]
openxr_extensions! {
    (VK_KHR_SWAPCHAIN, "VK_KHR_swapchain", 0),
    (VK_KHR_EXTERNAL_MEMORY, "VK_KHR_external_memory", 1),
    (VK_KHR_EXTERNAL_MEMORY_FD, "VK_KHR_external_memory_fd", 2),
    (VK_KHR_SURFACE, "VK_KHR_surface", 4),
    (VK_KHR_ANDROID_SURFACE, "VK_KHR_android_surface", 8),
    (VK_EXT_SWAPCHAIN_COLORSPACE, "VK_EXT_swapchain_colorspace", 16),
    (VK_KHR_GET_SURFACE_CAPABILITIES2, "VK_KHR_get_surface_capabilities2", 32),
    (VK_EXT_DEBUG_REPORT, "VK_EXT_debug_report", 64),
    (VK_KHR_GET_PHYSICAL_DEVICE_PROPERTIES2, "VK_KHR_get_physical_device_properties2", 128),
    (VK_KHR_EXTERNAL_SEMAPHORE_CAPABILITIES, "VK_KHR_external_semaphore_capabilities", 256),
    (VK_KHR_EXTERNAL_MEMORY_CAPABILITIES, "VK_KHR_external_memory_capabilities", 512),
    (VK_KHR_DEVICE_GROUP_CREATION, "VK_KHR_device_group_creation", 1024),
    (VK_KHR_EXTERNAL_FENCE_CAPABILITIES, "VK_KHR_external_fence_capabilities", 2048),
    (VK_KHR_DEDICATED_ALLOCATION, "VK_KHR_dedicated_allocation", 4096),
    (VK_KHR_EXTERNAL_FENCE, "VK_KHR_external_fence", 8192),
    (VK_KHR_EXTERNAL_FENCE_FD, "VK_KHR_external_fence_fd", 16384),
    (VK_KHR_EXTERNAL_SEMAPHORE, "VK_KHR_external_semaphore", 32768),
    (VK_KHR_EXTERNAL_SEMAPHORE_FD, "VK_KHR_external_semaphore_fd", 65536),
    (VK_KHR_GET_MEMORY_REQUIREMENTS2, "VK_KHR_get_memory_requirements2", 131072),
    (VK_KHR_EXTERNAL_MEMORY_WIN32, "VK_KHR_external_memory_win32", 262144), // 2**18
    (VK_KHR_EXTERNAL_FENCE_WIN32, "VK_KHR_external_fence_win32", 524288), // 2**19
    (VK_KHR_EXTERNAL_SEMAPHORE_WIN32, "VK_KHR_external_semaphore_win32", 1048576),
    (VK_NV_EXTERNAL_MEMORY_CAPABILITIES, "VK_NV_external_memory_capabilities", 2097152),
    (VK_KHR_WIN32_KEYED_MUTEX, "VK_KHR_win32_keyed_mutex", 4194304),
    (VK_EXT_DEBUG_MARKER, "VK_EXT_debug_marker", 8388608), // 2**23
}
