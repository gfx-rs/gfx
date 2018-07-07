use std::ptr;
#[cfg(feature= "winit")]
use std::sync::Mutex;
use std::sync::Arc;
#[cfg(feature= "winit")]
use std::ops::Deref;
use std::os::raw::c_void;

use ash::vk;
use ash::extensions as ext;

use hal;
use hal::image::{NumSamples, Size};
use hal::format::Format;

#[cfg(feature = "winit")]
use winit;

use conv;
use {VK_ENTRY, Backend, Instance, PhysicalDevice, QueueFamily, RawInstance};


pub struct Surface {
    // Vk (EXT) specs [29.2.7 Platform-Independent Information]
    // For vkDestroySurfaceKHR: Host access to surface must be externally synchronized
    pub(crate) raw: Arc<RawSurface>,
    pub(crate) width: Size,
    pub(crate) height: Size,
    pub(crate) samples: NumSamples,
}

pub struct RawSurface {
    pub(crate) handle: vk::SurfaceKHR,
    functor: ext::Surface,
    pub(crate) instance: Arc<RawInstance>,
}

impl Drop for RawSurface {
    fn drop(&mut self) {
        unsafe {
            self.functor.destroy_surface_khr(self.handle, None);
        }
    }
}

#[cfg(feature = "winit")]
pub struct Window {
    window: winit::Window,
}

#[cfg(feature = "winit")]
impl Window {
    pub fn new(wb: winit::WindowBuilder, el: Arc<Mutex<winit::EventsLoop>>) -> Arc<Mutex<Window>> {
        Arc::new(Mutex::new(Window {
            window: wb.build(&el.lock().unwrap()).unwrap(),
        }))
    }

    pub fn get_inner_size(&self) -> Option<winit::dpi::PhysicalSize> {
        self.window
            .get_inner_size()
            .map(|s| s.to_physical(self.window.get_hidpi_factor()))
    }
}

#[cfg(feature = "winit")]
impl Deref for Window {
    type Target = winit::Window;
    fn deref(&self) -> &Self::Target {
        &self.window
    }
}


impl Instance {
    #[cfg(all(unix, not(target_os = "android")))]
    pub fn create_surface_from_xlib(
        &self, dpy: *mut vk::Display, window: vk::Window
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME) {
            panic!("Vulkan driver does not support VK_KHR_XLIB_SURFACE");
        }

        let surface = {
            let xlib_loader = ext::XlibSurface::new(entry, &self.raw.0)
                .expect("XlibSurface::new() failed");

            let info = vk::XlibSurfaceCreateInfoKHR {
                s_type: vk::StructureType::XlibSurfaceCreateInfoKhr,
                p_next: ptr::null(),
                flags: vk::XlibSurfaceCreateFlagsKHR::empty(),
                window,
                dpy,
            };

            unsafe { xlib_loader.create_xlib_surface_khr(&info, None) }
                .expect("XlibSurface::create_xlib_surface_khr() failed")
        };

        let (width, height) = unsafe {
            use x11::xlib::{XGetWindowAttributes, XWindowAttributes};
            use std::mem::zeroed;
            let mut attribs: XWindowAttributes = zeroed();
            let result = XGetWindowAttributes(dpy as _, window, &mut attribs);
            if result == 0 {
                panic!("XGetGeometry failed");
            }
            (attribs.width as Size, attribs.height as Size)
        };

        self.create_surface_from_vk_surface_khr(surface, width, height, 1)
    }

    #[cfg(all(unix, not(target_os = "android")))]
    pub fn create_surface_from_xcb(
        &self, connection: *mut vk::xcb_connection_t, window: vk::xcb_window_t
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&vk::VK_KHR_XCB_SURFACE_EXTENSION_NAME) {
            panic!("Vulkan driver does not support VK_KHR_XCB_SURFACE");
        }

        let surface = {
            let xcb_loader = ext::XcbSurface::new(entry, &self.raw.0)
                .expect("XcbSurface::new() failed");

            let info = vk::XcbSurfaceCreateInfoKHR {
                s_type: vk::StructureType::XcbSurfaceCreateInfoKhr,
                p_next: ptr::null(),
                flags: vk::XcbSurfaceCreateFlagsKHR::empty(),
                window,
                connection,
            };

            unsafe { xcb_loader.create_xcb_surface_khr(&info, None) }
                .expect("XcbSurface::create_xcb_surface_khr() failed")
        };

        let (width, height) = unsafe {
            use std::mem;
            use xcb::{Connection, xproto};
            let conn = Connection::from_raw_conn(connection as _);
            let geometry = xproto::get_geometry(&conn, window)
                .get_reply()
                .expect("xcb_get_geometry failed")
                .ptr
                .as_ref()
                .expect("unexpected NULL XCB geometry");
            mem::forget(conn); //TODO: use `into_raw_conn`
            (geometry.width as _, geometry.height as _)
        };

        self.create_surface_from_vk_surface_khr(surface, width, height, 1)
    }

    #[cfg(all(unix, not(target_os = "android")))]
    pub fn create_surface_from_wayland(
        &self, display: *mut c_void, surface: *mut c_void, width: Size, height: Size
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&vk::VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME) {
            panic!("Vulkan driver does not support VK_KHR_WAYLAND_SURFACE");
        }

        let surface = {
            let w_loader = ext::WaylandSurface::new(entry, &self.raw.0)
                .expect("WaylandSurface failed");

            let info = vk::WaylandSurfaceCreateInfoKHR {
                s_type: vk::StructureType::WaylandSurfaceCreateInfoKhr,
                p_next: ptr::null(),
                flags: vk::WaylandSurfaceCreateFlagsKHR::empty(),
                display: display as *mut _,
                surface: surface as *mut _,
            };

            unsafe { w_loader.create_wayland_surface_khr(&info, None) }
                .expect("WaylandSurface failed")
        };

        self.create_surface_from_vk_surface_khr(surface, width, height, 1)
    }

    #[cfg(target_os = "android")]
    pub fn create_surface_android(
        &self, window: *const c_void, width: Size, height: Size
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        let surface = {
            let loader = ext::AndroidSurface::new(entry, &self.raw.0)
                .expect("AndroidSurface failed");

            let info = vk::AndroidSurfaceCreateInfoKHR {
                s_type: vk::StructureType::AndroidSurfaceCreateInfoKhr,
                p_next: ptr::null(),
                flags: vk::AndroidSurfaceCreateFlagsKHR::empty(),
                window: window as *const _ as *mut _,
            };

            unsafe { loader.create_android_surface_khr(&info, None) }
                .expect("AndroidSurface failed")
        };

        self.create_surface_from_vk_surface_khr(surface, width, height, 1)
    }

    #[cfg(windows)]
    pub fn create_surface_from_hwnd(
        &self, hinstance: *mut c_void, hwnd: *mut c_void
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&vk::VK_KHR_WIN32_SURFACE_EXTENSION_NAME) {
            panic!("Vulkan driver does not support VK_KHR_WIN32_SURFACE");
        }

        let surface = {
            let win32_loader = ext::Win32Surface::new(entry, &self.raw.0)
                .expect("Unable to load win32 surface functions");

            unsafe {
                let info = vk::Win32SurfaceCreateInfoKHR {
                    s_type: vk::StructureType::Win32SurfaceCreateInfoKhr,
                    p_next: ptr::null(),
                    flags: vk::Win32SurfaceCreateFlagsKHR::empty(),
                    hinstance: hinstance as *mut _,
                    hwnd: hwnd as *mut _,
                };

                win32_loader.create_win32_surface_khr(&info, None)
                    .expect("Unable to create Win32 surface")
            }
        };

        let (width, height) = unsafe {
            use winapi::shared::windef::RECT;
            use winapi::um::winuser::GetClientRect;
            use std::mem::zeroed;

            let mut rect: RECT = zeroed();
            if GetClientRect(hwnd as *mut _, &mut rect as *mut RECT) == 0 {
                panic!("GetClientRect failed");
            }
            ((rect.right - rect.left) as Size, (rect.bottom - rect.top) as Size)
        };

        self.create_surface_from_vk_surface_khr(surface, width, height, 1)
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(&self, window: &Arc<Mutex<Window>>) -> Surface {
        let window = &window.lock().unwrap().window;
        #[cfg(all(unix, not(target_os = "android")))]
        {
            use winit::os::unix::WindowExt;

            if self.extensions.contains(&vk::VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME) {
                if let Some(display) = window.get_wayland_display() {
                    let display: *mut c_void = display as *mut _;
                    let surface: *mut c_void = window.get_wayland_surface().unwrap() as *mut _;
                    let px = window
                        .get_inner_size()
                        .unwrap()
                        .to_physical(window.get_hidpi_factor());
                    return self.create_surface_from_wayland(display, surface, px.width as _, px.height as _);
                }
            }
            if self.extensions.contains(&vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME) {
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
            let (width, height) = window.get_inner_size().unwrap();
            self.create_surface_android(window.get_native_window(), width, height)

        }
        #[cfg(windows)]
        {
            use winapi::um::libloaderapi::GetModuleHandleW;
            use winit::os::windows::WindowExt;

            let hinstance = unsafe { GetModuleHandleW(ptr::null()) };
            let hwnd = window.get_hwnd();
            self.create_surface_from_hwnd(hinstance as *mut _, hwnd as *mut _)
        }
    }

    #[cfg(feature = "winit")]
    pub fn create_window(&self, wb: winit::WindowBuilder) -> Arc<Mutex<Window>> {
        Arc::new(Mutex::new(Window { window: wb.build(&self.el.lock().unwrap()).unwrap() }))
    }

    fn create_surface_from_vk_surface_khr(
        &self, surface: vk::SurfaceKHR, width: Size, height: Size, samples: NumSamples
    ) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        let functor = ext::Surface::new(entry, &self.raw.0)
            .expect("Unable to load surface functions");

        let raw = Arc::new(RawSurface {
            handle: surface,
            functor,
            instance: self.raw.clone(),
        });

        Surface { raw, width, height, samples }
    }
}

impl hal::Surface<Backend> for Surface {
    fn kind(&self) -> hal::image::Kind {
        hal::image::Kind::D2(self.width, self.height, 1, self.samples)
    }

    fn compatibility(
        &self, physical_device: &PhysicalDevice
    ) -> (hal::SurfaceCapabilities, Option<Vec<Format>>, Vec<hal::PresentMode>) {
        // Capabilities
        let caps =
            self.raw.functor.get_physical_device_surface_capabilities_khr(
                physical_device.handle,
                self.raw.handle,
            )
            .expect("Unable to query surface capabilities");

        // If image count is 0, the support number of images is unlimited.
        let max_images = if caps.max_image_count == 0 { !0 } else { caps.max_image_count };

        // `0xFFFFFFFF` indicates that the extent depends on the created swapchain.
        let current_extent =
            if caps.current_extent.width != 0xFFFFFFFF && caps.current_extent.height != 0xFFFFFFFF {
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
            image_count: caps.min_image_count..max_images,
            current_extent,
            extents: min_extent..max_extent,
            max_image_layers: caps.max_image_array_layers as _,
        };

        // Swapchain formats
        let formats =
            self.raw.functor.get_physical_device_surface_formats_khr(
                physical_device.handle,
                self.raw.handle,
            ).expect("Unable to query surface formats");

        let formats = match formats[0].format {
            // If pSurfaceFormats includes just one entry, whose value for format is
            // VK_FORMAT_UNDEFINED, surface has no preferred format. In this case, the application
            // can use any valid VkFormat value.
            vk::Format::Undefined => None,
            _ => {
                Some(formats
                    .into_iter()
                    .filter_map(|sf| conv::map_vk_format(sf.format))
                    .collect()
                )
            }
        };

        let present_modes =
            self.raw.functor.get_physical_device_surface_present_modes_khr(
                physical_device.handle,
                self.raw.handle,
            ).expect("Unable to query present modes");
        let present_modes = present_modes
            .into_iter()
            .map(conv::map_vk_present_mode)
            .collect();

        (capabilities, formats, present_modes)
    }

    fn supports_queue_family(&self, queue_family: &QueueFamily) -> bool {
        self.raw.functor.get_physical_device_surface_support_khr(
            queue_family.device,
            queue_family.index,
            self.raw.handle,
        )
    }
}

pub struct Swapchain {
    pub(crate) raw: vk::SwapchainKHR,
    pub(crate) functor: ext::Swapchain,
}


impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_image(&mut self, sync: hal::FrameSync<Backend>) -> Result<hal::SwapImageIndex, ()> {
        let (semaphore, fence) = match sync {
            hal::FrameSync::Semaphore(semaphore) => (semaphore.0, vk::Fence::null()),
            hal::FrameSync::Fence(fence) => (vk::Semaphore::null(), fence.0),
        };

        let index = unsafe {
            // will block if no image is available
            self.functor.acquire_next_image_khr(self.raw, !0, semaphore, fence)
        };

        match index {
            Ok(i) => Ok(i),
            Err(vk::Result::SuboptimalKhr) | Err(vk::Result::ErrorOutOfDateKhr) => Err(()),
            _ => panic!("Failed to acquire image."),
        }
    }
}
