use std::os::raw::c_void;
use std::ptr;
use std::sync::Arc;

use ash::extensions::khr;
use ash::vk;

use crate::hal;
use crate::hal::format::Format;

#[cfg(feature = "winit")]
use winit;

use crate::{conv, native};
use crate::{Backend, Instance, PhysicalDevice, QueueFamily, RawInstance, VK_ENTRY};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Surface {
    // Vk (EXT) specs [29.2.7 Platform-Independent Information]
    // For vkDestroySurfaceKHR: Host access to surface must be externally synchronized
    #[derivative(Debug = "ignore")]
    pub(crate) raw: Arc<RawSurface>,
}

pub struct RawSurface {
    pub(crate) handle: vk::SurfaceKHR,
    functor: khr::Surface,
    pub(crate) instance: Arc<RawInstance>,
}

impl Drop for RawSurface {
    fn drop(&mut self) {
        unsafe {
            self.functor.destroy_surface(self.handle, None);
        }
    }
}

impl Instance {
    #[cfg(all(
        feature = "x11",
        unix,
        not(target_os = "android"),
        not(target_os = "macos")
    ))]
    pub fn create_surface_from_xlib(&self, dpy: *mut vk::Display, window: vk::Window) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&khr::XlibSurface::name()) {
            panic!("Vulkan driver does not support VK_KHR_XLIB_SURFACE");
        }

        let surface = {
            let xlib_loader = khr::XlibSurface::new(entry, &self.raw.0);
            let info = vk::XlibSurfaceCreateInfoKHR {
                s_type: vk::StructureType::XLIB_SURFACE_CREATE_INFO_KHR,
                p_next: ptr::null(),
                flags: vk::XlibSurfaceCreateFlagsKHR::empty(),
                window,
                dpy,
            };

            unsafe { xlib_loader.create_xlib_surface(&info, None) }
                .expect("XlibSurface::create_xlib_surface() failed")
        };

        self.create_surface_from_vk_surface_khr(surface)
    }

    #[cfg(all(
        feature = "xcb",
        unix,
        not(target_os = "android"),
        not(target_os = "macos")
    ))]
    pub fn create_surface_from_xcb(
        &self,
        connection: *mut vk::xcb_connection_t,
        window: vk::xcb_window_t,
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&khr::XcbSurface::name()) {
            panic!("Vulkan driver does not support VK_KHR_XCB_SURFACE");
        }

        let surface = {
            let xcb_loader = khr::XcbSurface::new(entry, &self.raw.0);
            let info = vk::XcbSurfaceCreateInfoKHR {
                s_type: vk::StructureType::XCB_SURFACE_CREATE_INFO_KHR,
                p_next: ptr::null(),
                flags: vk::XcbSurfaceCreateFlagsKHR::empty(),
                window,
                connection,
            };

            unsafe { xcb_loader.create_xcb_surface(&info, None) }
                .expect("XcbSurface::create_xcb_surface() failed")
        };

        self.create_surface_from_vk_surface_khr(surface)
    }

    #[cfg(all(unix, not(target_os = "android")))]
    pub fn create_surface_from_wayland(
        &self,
        display: *mut c_void,
        surface: *mut c_void,
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&khr::WaylandSurface::name()) {
            panic!("Vulkan driver does not support VK_KHR_WAYLAND_SURFACE");
        }

        let surface = {
            let w_loader = khr::WaylandSurface::new(entry, &self.raw.0);
            let info = vk::WaylandSurfaceCreateInfoKHR {
                s_type: vk::StructureType::WAYLAND_SURFACE_CREATE_INFO_KHR,
                p_next: ptr::null(),
                flags: vk::WaylandSurfaceCreateFlagsKHR::empty(),
                display: display as *mut _,
                surface: surface as *mut _,
            };

            unsafe { w_loader.create_wayland_surface(&info, None) }.expect("WaylandSurface failed")
        };

        self.create_surface_from_vk_surface_khr(surface)
    }

    #[cfg(target_os = "android")]
    pub fn create_surface_android(
        &self,
        window: *const c_void,
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        let surface = {
            let loader = khr::AndroidSurface::new(entry, &self.raw.0);
            let info = vk::AndroidSurfaceCreateInfoKHR {
                s_type: vk::StructureType::ANDROID_SURFACE_CREATE_INFO_KHR,
                p_next: ptr::null(),
                flags: vk::AndroidSurfaceCreateFlagsKHR::empty(),
                window: window as *const _ as *mut _,
            };

            unsafe { loader.create_android_surface(&info, None) }.expect("AndroidSurface failed")
        };

        self.create_surface_from_vk_surface_khr(surface)
    }

    #[cfg(windows)]
    pub fn create_surface_from_hwnd(&self, hinstance: *mut c_void, hwnd: *mut c_void) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&khr::Win32Surface::name()) {
            panic!("Vulkan driver does not support VK_KHR_WIN32_SURFACE");
        }

        let surface = {
            let info = vk::Win32SurfaceCreateInfoKHR {
                s_type: vk::StructureType::WIN32_SURFACE_CREATE_INFO_KHR,
                p_next: ptr::null(),
                flags: vk::Win32SurfaceCreateFlagsKHR::empty(),
                hinstance: hinstance as *mut _,
                hwnd: hwnd as *mut _,
            };
            let win32_loader = khr::Win32Surface::new(entry, &self.raw.0);
            unsafe {
                win32_loader
                    .create_win32_surface(&info, None)
                    .expect("Unable to create Win32 surface")
            }
        };

        self.create_surface_from_vk_surface_khr(surface)
    }

    #[cfg(target_os = "macos")]
    pub fn create_surface_from_nsview(&self, view: *mut c_void) -> Surface {
        use ash::extensions::mvk;
        use core_graphics::{base::CGFloat, geometry::CGRect};
        use objc::runtime::{Object, BOOL, YES};

        // TODO: this logic is duplicated from gfx-backend-metal, refactor?
        unsafe {
            let view = view as *mut Object;
            let existing: *mut Object = msg_send![view, layer];
            let class = class!(CAMetalLayer);

            let use_current = if existing.is_null() {
                false
            } else {
                let result: BOOL = msg_send![existing, isKindOfClass: class];
                result == YES
            };

            if !use_current {
                let layer: *mut Object = msg_send![class, new];
                msg_send![view, setLayer: layer];
                let bounds: CGRect = msg_send![view, bounds];
                msg_send![layer, setBounds: bounds];

                let window: *mut Object = msg_send![view, window];
                if !window.is_null() {
                    let scale_factor: CGFloat = msg_send![window, backingScaleFactor];
                    msg_send![layer, setContentsScale: scale_factor];
                }
            }
        }

        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&mvk::MacOSSurface::name()) {
            panic!("Vulkan driver does not support VK_MVK_MACOS_SURFACE");
        }

        let surface = {
            let mac_os_loader = mvk::MacOSSurface::new(entry, &self.raw.0);
            let info = vk::MacOSSurfaceCreateInfoMVK {
                s_type: vk::StructureType::MACOS_SURFACE_CREATE_INFO_M,
                p_next: ptr::null(),
                flags: vk::MacOSSurfaceCreateFlagsMVK::empty(),
                p_view: view,
            };

            unsafe {
                mac_os_loader
                    .create_mac_os_surface_mvk(&info, None)
                    .expect("Unable to create macOS surface")
            }
        };

        self.create_surface_from_vk_surface_khr(surface)
    }

    #[cfg(feature = "winit")]
    #[allow(unreachable_code)]
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        #[cfg(all(
            feature = "x11",
            unix,
            not(target_os = "android"),
            not(target_os = "macos")
        ))]
        {
            use winit::os::unix::WindowExt;

            if self.extensions.contains(&khr::WaylandSurface::name()) {
                if let Some(display) = window.get_wayland_display() {
                    let display: *mut c_void = display as *mut _;
                    let surface: *mut c_void = window.get_wayland_surface().unwrap() as *mut _;
                    return self.create_surface_from_wayland(
                        display,
                        surface,
                    );
                }
            }
            if self.extensions.contains(&khr::XlibSurface::name()) {
                if let Some(display) = window.get_xlib_display() {
                    let window = window.get_xlib_window().unwrap();
                    return self.create_surface_from_xlib(display as _, window);
                }
            }
            panic!("The Vulkan driver does not support surface creation!");
        }
        #[cfg(target_os = "android")]
        {
            use winit::os::android::WindowExt;
            return self.create_surface_android(
                window.get_native_window(),
            );
        }
        #[cfg(windows)]
        {
            use winapi::um::libloaderapi::GetModuleHandleW;
            use winit::os::windows::WindowExt;

            let hinstance = unsafe { GetModuleHandleW(ptr::null()) };
            let hwnd = window.get_hwnd();
            return self.create_surface_from_hwnd(hinstance as *mut _, hwnd as *mut _);
        }
        #[cfg(target_os = "macos")]
        {
            use winit::os::macos::WindowExt;

            return self.create_surface_from_nsview(window.get_nsview());
        }
        let _ = window;
        panic!("No suitable WSI enabled!");
    }

    pub fn create_surface_from_raw(
        &self,
        has_handle: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, hal::window::InitError> {
        use raw_window_handle::RawWindowHandle;

        match has_handle.raw_window_handle() {
            #[cfg(all(
                feature = "x11",
                unix,
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            RawWindowHandle::Wayland(handle)
                if self.extensions.contains(&khr::WaylandSurface::name()) =>
            {
                Ok(self.create_surface_from_wayland(handle.display, handle.surface))
            }
            #[cfg(all(
                feature = "x11",
                unix,
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            RawWindowHandle::Xlib(handle)
                if self.extensions.contains(&khr::XlibSurface::name()) =>
            {
                Ok(self.create_surface_from_xlib(handle.display as *mut _, handle.window))
            }
            // #[cfg(target_os = "android")]
            // RawWindowHandle::ANativeWindowHandle(handle) => {
            //     let native_window = unimplemented!();
            //     self.create_surface_android(native_window)
            //}
            #[cfg(windows)]
            RawWindowHandle::Windows(handle) => {
                use winapi::um::libloaderapi::GetModuleHandleW;

                let hinstance = unsafe { GetModuleHandleW(ptr::null()) };
                Ok(self.create_surface_from_hwnd(hinstance as *mut _, handle.hwnd))
            }
            #[cfg(target_os = "macos")]
            RawWindowHandle::MacOS(handle) => {
                Ok(self.create_surface_from_nsview(handle.ns_view))
            }
            _ => Err(hal::window::InitError::UnsupportedWindowHandle),
        }
    }

    pub fn create_surface_from_vk_surface_khr(
        &self,
        surface: vk::SurfaceKHR,
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        let functor = khr::Surface::new(entry, &self.raw.0);

        let raw = Arc::new(RawSurface {
            handle: surface,
            functor,
            instance: self.raw.clone(),
        });

        Surface {
            raw,
        }
    }
}

impl hal::Surface<Backend> for Surface {
    fn compatibility(
        &self,
        physical_device: &PhysicalDevice,
    ) -> (
        hal::SurfaceCapabilities,
        Option<Vec<Format>>,
        Vec<hal::PresentMode>,
    ) {
        // Capabilities
        let caps = unsafe {
            self.raw
                .functor
                .get_physical_device_surface_capabilities(physical_device.handle, self.raw.handle)
        }
        .expect("Unable to query surface capabilities");

        // If image count is 0, the support number of images is unlimited.
        let max_images = if caps.max_image_count == 0 {
            !0
        } else {
            caps.max_image_count
        };

        // `0xFFFFFFFF` indicates that the extent depends on the created swapchain.
        let current_extent = if caps.current_extent.width != !0 && caps.current_extent.height != !0
        {
            Some(hal::window::Extent2D {
                width: caps.current_extent.width,
                height: caps.current_extent.height,
            })
        } else {
            None
        };

        let min_extent = hal::window::Extent2D {
            width: caps.min_image_extent.width,
            height: caps.min_image_extent.height,
        };

        let max_extent = hal::window::Extent2D {
            width: caps.max_image_extent.width,
            height: caps.max_image_extent.height,
        };

        let capabilities = hal::SurfaceCapabilities {
            image_count: caps.min_image_count ..= max_images,
            current_extent,
            extents: min_extent ..= max_extent,
            max_image_layers: caps.max_image_array_layers as _,
            usage: conv::map_vk_image_usage(caps.supported_usage_flags),
            composite_alpha: conv::map_vk_composite_alpha(caps.supported_composite_alpha),
        };

        // Swapchain formats
        let formats = unsafe {
            self.raw
                .functor
                .get_physical_device_surface_formats(physical_device.handle, self.raw.handle)
        }
        .expect("Unable to query surface formats");

        let formats = match formats[0].format {
            // If pSurfaceFormats includes just one entry, whose value for format is
            // VK_FORMAT_UNDEFINED, surface has no preferred format. In this case, the application
            // can use any valid VkFormat value.
            vk::Format::UNDEFINED => None,
            _ => Some(
                formats
                    .into_iter()
                    .filter_map(|sf| conv::map_vk_format(sf.format))
                    .collect(),
            ),
        };

        let present_modes = unsafe {
            self.raw
                .functor
                .get_physical_device_surface_present_modes(physical_device.handle, self.raw.handle)
        }
        .expect("Unable to query present modes");
        let present_modes = present_modes
            .into_iter()
            .map(conv::map_vk_present_mode)
            .collect();

        (capabilities, formats, present_modes)
    }

    fn supports_queue_family(&self, queue_family: &QueueFamily) -> bool {
        unsafe {
            self.raw.functor.get_physical_device_surface_support(
                queue_family.device,
                queue_family.index,
                self.raw.handle,
            )
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Swapchain {
    pub(crate) raw: vk::SwapchainKHR,
    #[derivative(Debug = "ignore")]
    pub(crate) functor: khr::Swapchain,
}

impl hal::Swapchain<Backend> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        timeout_ns: u64,
        semaphore: Option<&native::Semaphore>,
        fence: Option<&native::Fence>,
    ) -> Result<(hal::SwapImageIndex, Option<hal::window::Suboptimal>), hal::AcquireError> {
        let semaphore = semaphore.map_or(vk::Semaphore::null(), |s| s.0);
        let fence = fence.map_or(vk::Fence::null(), |f| f.0);

        // will block if no image is available
        let index = self
            .functor
            .acquire_next_image(self.raw, timeout_ns, semaphore, fence);

        match index {
            Ok((i, suboptimal)) => {
                if suboptimal {
                    Ok((i, Some(hal::window::Suboptimal)))
                } else {
                    Ok((i, None))
                }
            }
            Err(vk::Result::NOT_READY) => Err(hal::AcquireError::NotReady),
            Err(vk::Result::TIMEOUT) => Err(hal::AcquireError::Timeout),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Err(hal::AcquireError::OutOfDate),
            Err(vk::Result::ERROR_SURFACE_LOST_KHR) => {
                Err(hal::AcquireError::SurfaceLost(hal::device::SurfaceLost))
            }
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(hal::AcquireError::OutOfMemory(
                hal::device::OutOfMemory::OutOfHostMemory,
            )),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(hal::AcquireError::OutOfMemory(
                hal::device::OutOfMemory::OutOfDeviceMemory,
            )),
            Err(vk::Result::ERROR_DEVICE_LOST) => {
                Err(hal::AcquireError::DeviceLost(hal::device::DeviceLost))
            }
            _ => panic!("Failed to acquire image."),
        }
    }
}
