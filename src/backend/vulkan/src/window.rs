use std::collections::VecDeque;
use std::ptr;
use std::sync::Arc;
use std::os::raw::c_void;

use ash::vk;
use ash::extensions as ext;

use hal;

#[cfg(feature = "winit")]
use winit;

use {conv, native};
use {VK_ENTRY, Adapter, Backend, Instance, ProtoQueueFamily, RawInstance};


pub struct Surface {
    // Vk (EXT) specs [29.2.7 Platform-Independent Information]
    // For vkDestroySurfaceKHR: Host access to surface must be externally synchronized
    raw: Arc<RawSurface>,
    width: u32,
    height: u32,
}

pub struct RawSurface {
    handle: vk::SurfaceKHR,
    functor: ext::Surface,
    instance: Arc<RawInstance>,
}

impl Drop for RawSurface {
    fn drop(&mut self) {
        unsafe {
            self.functor.destroy_surface_khr(self.handle, None);
        }
    }
}

impl Instance {
    #[cfg(unix)]
    pub fn create_surface_from_xlib(&self, display: *mut c_void, window: usize) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME) {
            panic!("Vulkan driver does not support VK_KHR_XLIB_SURFACE");
        }

        let surface = {
            let xlib_loader = ext::XlibSurface::new(entry, &self.raw.0)
                                .expect("XlibSurface failed");

            let info = vk::XlibSurfaceCreateInfoKHR {
                s_type: vk::StructureType::XlibSurfaceCreateInfoKhr,
                p_next: ptr::null(),
                flags: vk::XlibSurfaceCreateFlagsKHR::empty(),
                window: window as _,
                dpy: display as *mut _,
            };

            unsafe { xlib_loader.create_xlib_surface_khr(&info, None) }
                .expect("XlibSurface failed")
        };

        let (width, height) = unsafe {
            use x11::xlib::{XGetWindowAttributes, XWindowAttributes};
            use std::mem::zeroed;
            let mut attribs: XWindowAttributes = zeroed();
            let result = XGetWindowAttributes(display as *mut _, window as _, &mut attribs);
            if result == 0 {
                panic!("XGetGeometry failed");
            }
            (attribs.width as u32, attribs.height as u32)
        };

        self.create_surface_from_vk_surface_khr(surface, width, height)
    }

    #[cfg(unix)]
    pub fn create_surface_from_wayland(&self, display: *mut c_void, surface: *mut c_void, width: u32, height: u32) -> Surface {
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

        self.create_surface_from_vk_surface_khr(surface, width, height)
    }

    #[cfg(windows)]
    pub fn create_surface_from_hwnd(&self, hwnd: *mut c_void) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        if !self.extensions.contains(&vk::VK_KHR_WIN32_SURFACE_EXTENSION_NAME) {
            panic!("Vulkan driver does not support VK_KHR_WIN32_SURFACE");
        }

        let surface = {
            use kernel32;

            let win32_loader = ext::Win32Surface::new(entry, &self.raw.0)
                .expect("Unable to load win32 surface functions");

            unsafe {
                let info = vk::Win32SurfaceCreateInfoKHR {
                    s_type: vk::StructureType::Win32SurfaceCreateInfoKhr,
                    p_next: ptr::null(),
                    flags: vk::Win32SurfaceCreateFlagsKHR::empty(),
                    hinstance: kernel32::GetModuleHandleW(ptr::null()) as *mut _,
                    hwnd: hwnd as *mut _,
                };

                win32_loader.create_win32_surface_khr(&info, None)
                    .expect("Unable to create Win32 surface")
            }
        };

        let (width, height) = unsafe {
            use winapi::RECT;
            use user32::GetClientRect;
            use std::mem::zeroed;
            let mut rect: RECT = zeroed();
            if GetClientRect(hwnd as *mut _, &mut rect as *mut RECT) == 0 {
                panic!("GetClientRect failed");
            }
            ((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32)
        };

        self.create_surface_from_vk_surface_khr(surface, width, height)
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        #[cfg(unix)]
        {
            use winit::os::unix::WindowExt;

            if self.extensions.contains(&vk::VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME) {
                if let Some(display) = window.get_wayland_display() {
                    let display: *mut c_void = display as *mut _;
                    let surface: *mut c_void = window.get_wayland_surface().unwrap() as *mut _;
                    let (width, height) = window.get_inner_size().unwrap();
                    return self.create_surface_from_wayland(display, surface, width, height);
                }
            }
            if self.extensions.contains(&vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME) {
                if let Some(display) = window.get_xlib_display() {
                    let display: *mut c_void = display as *mut _;
                    let window: usize = window.get_xlib_window().unwrap() as _;
                    return self.create_surface_from_xlib(display, window);
                }
            }
            panic!("The Vulkan driver does not support surface creation!");
        }
        #[cfg(windows)]
        {
            use winit::os::windows::WindowExt;
            self.create_surface_from_hwnd(window.get_hwnd() as *mut _)
        }
    }

    fn create_surface_from_vk_surface_khr(&self, surface: vk::SurfaceKHR, width: u32, height: u32) -> Surface {
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

        Surface { raw, width, height }
    }
}

impl hal::Surface<Backend> for Surface {
    fn get_kind(&self) -> hal::image::Kind {
        use hal::image::Size;

        let aa = hal::image::AaMode::Single;
        hal::image::Kind::D2(self.width as Size, self.height as Size, aa)
    }

    fn surface_capabilities(&self, adapter: &Adapter) -> hal::SurfaceCapabilities {
        let caps =
            self.raw.functor.get_physical_device_surface_capabilities_khr(
                adapter.handle,
                self.raw.handle,
            )
            .expect("Unable to query surface capabilities");

        // If image count is 0, the support number of images is unlimited.
        let max_images = if caps.max_image_count == 0 { !0 } else { caps.max_image_count };

        // `0xFFFFFFFF` indicates that the extent depends on the created swapchain.
        let current_extent =
            if caps.current_extent.width != 0xFFFFFFFF && caps.current_extent.height != 0xFFFFFFFF {
                Some(hal::window::Extent2d {
                    width: caps.current_extent.width,
                    height: caps.current_extent.height,
                })
            } else {
                None
            };

        let min_extent = hal::window::Extent2d {
            width: caps.min_image_extent.width,
            height: caps.min_image_extent.height,
        };

        let max_extent = hal::window::Extent2d {
            width: caps.max_image_extent.width,
            height: caps.max_image_extent.height,
        };

        hal::SurfaceCapabilities {
            image_count: caps.min_image_count..max_images,
            current_extent,
            extents: min_extent..max_extent,
            max_image_layers: caps.max_image_array_layers,
        }
    }

    fn supports_queue_family(&self, queue_family: &ProtoQueueFamily) -> bool {
        self.raw.functor.get_physical_device_surface_support_khr(
            queue_family.device,
            queue_family.index,
            self.raw.handle,
        )
    }

    fn build_swapchain<C>(
        &mut self,
        config: hal::SwapchainConfig,
        present_queue: &hal::CommandQueue<Backend, C>,
    ) -> (Swapchain, hal::Backbuffer<Backend>) {
        let functor = ext::Swapchain::new(&self.raw.instance.0, &present_queue.as_raw().device.0)
            .expect("Unable to query swapchain function");

        // TODO: check for better ones if available
        let present_mode = vk::PresentModeKHR::Fifo; // required to be supported

        // TODO: handle depth stencil
        let format = config.color_format;

        let info = vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SwapchainCreateInfoKhr,
            p_next: ptr::null(),
            flags: vk::SwapchainCreateFlagsKHR::empty(),
            surface: self.raw.handle,
            min_image_count: 2, // TODO: let the user specify the value
            image_format: conv::map_format(format.0, format.1).unwrap(),
            image_color_space: vk::ColorSpaceKHR::SrgbNonlinear,
            image_extent: vk::Extent2D {
                width: self.width,
                height: self.height,
            },
            image_array_layers: 1,
            image_usage: vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
            image_sharing_mode: vk::SharingMode::Exclusive,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            pre_transform: vk::SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
            composite_alpha: vk::COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
            present_mode: present_mode,
            clipped: 1,
            old_swapchain: vk::SwapchainKHR::null(),
        };

        let swapchain_raw = unsafe { functor.create_swapchain_khr(&info, None) }
            .expect("Unable to create a swapchain");

        let backbuffer_images = functor.get_swapchain_images_khr(swapchain_raw)
            .expect("Unable to get swapchain images");

        let swapchain = Swapchain {
            raw: swapchain_raw,
            functor,
            frame_queue: VecDeque::new(),
        };

        let images = backbuffer_images
            .into_iter()
            .map(|image| {
                native::Image {
                    raw: image,
                    bytes_per_texel: 4,
                    extent: vk::Extent3D {
                        width: self.width,
                        height: self.height,
                        depth: 1,
                    },
                }
            })
            .collect();

        (swapchain, hal::Backbuffer::Images(images))
    }
}

pub struct Swapchain {
    raw: vk::SwapchainKHR,
    functor: ext::Swapchain,
    // Queued up frames for presentation
    frame_queue: VecDeque<usize>,
}


impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, sync: hal::FrameSync<Backend>) -> hal::Frame {
        let (semaphore, fence) = match sync {
            hal::FrameSync::Semaphore(semaphore) => (semaphore.0, vk::Fence::null()),
            hal::FrameSync::Fence(fence) => (vk::Semaphore::null(), fence.0),
        };

        let index = unsafe {
            // will block if no image is available
            self.functor.acquire_next_image_khr(self.raw, !0, semaphore, fence)
        }.expect("Unable to acquire a swapchain image");

        self.frame_queue.push_back(index as usize);
        hal::Frame::new(index as usize)
    }

    fn present<C>(
        &mut self,
        present_queue: &mut hal::CommandQueue<Backend, C>,
        wait_semaphores: &[&native::Semaphore],
    ) {
        let frame = self.frame_queue.pop_front().expect(
            "No frame currently queued up. Need to acquire a frame first.",
        );

        let semaphores = wait_semaphores.iter().map(|sem| sem.0).collect::<Vec<_>>();

        // TODO: wait semaphores
        let info = vk::PresentInfoKHR {
            s_type: vk::StructureType::PresentInfoKhr,
            p_next: ptr::null(),
            wait_semaphore_count: semaphores.len() as u32,
            p_wait_semaphores: semaphores.as_ptr(),
            swapchain_count: 1,
            p_swapchains: &self.raw,
            p_image_indices: &(frame as u32),
            p_results: ptr::null_mut(),
        };

        assert_eq!(Ok(()), unsafe {
            self.functor
                .queue_present_khr(*present_queue.as_raw().raw, &info)
        });
        // TODO: handle result and return code
    }
}
