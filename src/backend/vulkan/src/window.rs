use std::collections::VecDeque;
use std::ptr;
use std::sync::Arc;

use ash::vk;
use ash::extensions as ext;

use {core, winit};

use {conv, native};
use {VK_ENTRY, Backend, Instance, QueueFamily, RawInstance};


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
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        let entry = VK_ENTRY
            .as_ref()
            .expect("Unable to load Vulkan entry points");

        let surface = self
            .surface_extensions
            .iter()
            .map(|&extension| {
                match extension {
                    #[cfg(not(target_os = "windows"))]
                    vk::VK_KHR_XLIB_SURFACE_EXTENSION_NAME => {
                        use winit::os::unix::WindowExt;
                        let xlib_loader = match ext::XlibSurface::new(entry, &self.raw.0) {
                            Ok(loader) => loader,
                            Err(e) => {
                                error!("XlibSurface failed: {:?}", e);
                                return None;
                            }
                        };

                        let info = vk::XlibSurfaceCreateInfoKHR {
                            s_type: vk::StructureType::XlibSurfaceCreateInfoKhr,
                            p_next: ptr::null(),
                            flags: vk::XlibSurfaceCreateFlagsKHR::empty(),
                            window: window.get_xlib_window().unwrap() as *const _,
                            dpy: window.get_xlib_display().unwrap() as *mut _,
                        };

                        unsafe {
                            xlib_loader.create_xlib_surface_khr(&info, None).ok()
                        }
                    }
                    #[cfg(target_os = "windows")]
                    vk::VK_KHR_WIN32_SURFACE_EXTENSION_NAME => {
                        use winit::os::windows::WindowExt;
                        let win32_loader = ext::Win32Surface::new(entry, &instance.raw.0)
                            .expect("Unable to load win32 surface functions");

                        unsafe {
                            let info = vk::Win32SurfaceCreateInfoKHR {
                                s_type: vk::StructureType::Win32SurfaceCreateInfoKhr,
                                p_next: ptr::null(),
                                flags: vk::Win32SurfaceCreateFlagsKHR::empty(),
                                hinstance: kernel32::GetModuleHandleW(ptr::null()) as *mut _,
                                hwnd: window.get_hwnd() as *mut _,
                            };

                            win32_loader
                                .create_win32_surface_khr(&info, None)
                                .expect("Error on surface creation")
                        }
                    }
                    // TODO: other platforms
                    _ => None,
                }
            })
            .find(|x| x.is_some())
            .expect("Unable to find a surface implementation.")
            .unwrap();

        let (width, height) = window.get_inner_size_pixels().unwrap();
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

impl core::Surface<Backend> for Surface {
    type Swapchain = Swapchain;

    fn supports_queue(&self, queue_family: &QueueFamily) -> bool {
        self.raw.functor.get_physical_device_surface_support_khr(
            queue_family.device(),
            queue_family.family_index(), //Note: should be queue index?
            self.raw.handle,
        )
    }

    fn build_swapchain<C>(
        &mut self,
        config: core::SwapchainConfig,
        present_queue: &core::CommandQueue<Backend, C>,
    ) -> Self::Swapchain {
        let functor = ext::Swapchain::new(&self.raw.instance.0, &present_queue.as_raw().device().0)
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

        let swapchain = unsafe { functor.create_swapchain_khr(&info, None) }
            .expect("Unable to create a swapchain");

        let backbuffer_images = { functor.get_swapchain_images_khr(swapchain) }
            .expect("Unable to get swapchain images");

        let backbuffers = backbuffer_images
            .into_iter()
            .map(|image| {
                core::Backbuffer {
                    color: native::Image {
                        raw: image,
                        bytes_per_texel: 4,
                        extent: vk::Extent3D {
                            width: self.width,
                            height: self.height,
                            depth: 1,
                        },
                    },
                    depth_stencil: None,
                }
            })
            .collect();

        Swapchain {
            raw: swapchain,
            functor,
            backbuffers,
            frame_queue: VecDeque::new(),
        }
    }
}

pub struct Swapchain {
    raw: vk::SwapchainKHR,
    functor: ext::Swapchain,
    backbuffers: Vec<core::Backbuffer<Backend>>,
    // Queued up frames for presentation
    frame_queue: VecDeque<usize>,
}


impl core::Swapchain<Backend> for Swapchain {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<Backend>] {
        &self.backbuffers
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<Backend>) -> core::Frame {
        let (semaphore, fence) = match sync {
            core::FrameSync::Semaphore(semaphore) => (semaphore.0, vk::Fence::null()),
            core::FrameSync::Fence(fence) => (vk::Semaphore::null(), fence.0),
        };

        let index = unsafe {
            // will block if no image is available
            self.functor.acquire_next_image_khr(self.raw, !0, semaphore, fence)
        }.expect("Unable to acquire a swapchain image");

        self.frame_queue.push_back(index as usize);
        core::Frame::new(index as usize)
    }

    fn present<C>(
        &mut self,
        present_queue: &mut core::CommandQueue<Backend, C>,
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
                .queue_present_khr(*present_queue.as_raw().raw(), &info)
        });
        // TODO: handle result and return code
    }
}
