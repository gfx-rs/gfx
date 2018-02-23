use std::collections::VecDeque;
use std::mem;

#[cfg(feature = "winit")]
use winit;

use winapi::shared::dxgi1_4;
use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser::GetClientRect;
use wio::com::ComPtr;

use hal::{self, format as f, image as i};
use {native as n, Backend, Instance, PhysicalDevice, QueueFamily};

use std::os::raw::c_void;

impl Instance {
    pub fn create_surface_from_hwnd(&self, hwnd: *mut c_void) -> Surface {
        let (width, height) = unsafe {
            let mut rect: RECT = mem::zeroed();
            if GetClientRect(hwnd as *mut _, &mut rect as *mut RECT) == 0 {
                panic!("GetClientRect failed");
            }
            ((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32)
        };

        Surface {
            factory: self.factory.clone(),
            wnd_handle: hwnd as *mut _,
            width: width,
            height: height,
        }
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        use winit::os::windows::WindowExt;
        self.create_surface_from_hwnd(window.get_hwnd() as *mut _)
    }
}

pub struct Surface {
    pub(crate) factory: ComPtr<dxgi1_4::IDXGIFactory4>,
    pub(crate) wnd_handle: HWND,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

unsafe impl Send for Surface { }
unsafe impl Sync for Surface { }

impl hal::Surface<Backend> for Surface {
    fn supports_queue_family(&self, _queue_family: &QueueFamily) -> bool { true }
    fn kind(&self) -> i::Kind {
        let aa = i::AaMode::Single;
        i::Kind::D2(self.width as i::Size, self.height as i::Size, aa)
    }

    fn capabilities_and_formats(
        &self, _: &PhysicalDevice,
    ) -> (hal::SurfaceCapabilities, Option<Vec<f::Format>>) {
        let extent = hal::window::Extent2D {
            width: self.width,
            height: self.height,
        };

        let capabilities = hal::SurfaceCapabilities {
            image_count: 2..16, // we currently use a flip effect which supports 2..16 buffers
            current_extent: Some(extent),
            extents: extent..extent,
            max_image_layers: 1,
        };

        // Sticking to FLIP swap effects for the moment.
        // We also expose sRGB buffers but they are handled internally as UNORM.
        // Roughly ordered by popularity..
        let formats = vec![
            f::Format::Bgra8Srgb,
            f::Format::Bgra8Unorm,
            f::Format::Rgba8Srgb,
            f::Format::Rgba8Unorm,
            f::Format::A2b10g10r10Unorm,
            f::Format::Rgba16Float,
        ];

        (capabilities, Some(formats))
    }
}

pub struct Swapchain {
    pub(crate) inner: ComPtr<dxgi1_4::IDXGISwapChain3>,
    pub(crate) next_frame: usize,
    pub(crate) frame_queue: VecDeque<usize>,
    #[allow(dead_code)]
    pub(crate) rtv_heap: n::DescriptorHeap,
}

impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, _sync: hal::FrameSync<Backend>) -> hal::Frame {
        // TODO: sync

        if false {
            // TODO: we need to block this at some point? (running out of backbuffers)
            //let num_images = self.images.len();
            let num_images = 1;
            let index = self.next_frame;
            self.frame_queue.push_back(index);
            self.next_frame = (self.next_frame + 1) % num_images;
        }

        // TODO:
        let index = unsafe { self.inner.GetCurrentBackBufferIndex() };
        hal::Frame::new(index as usize)
    }
}

unsafe impl Send for Swapchain { }
unsafe impl Sync for Swapchain { }
