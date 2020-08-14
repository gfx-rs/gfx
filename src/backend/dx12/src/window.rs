use std::{collections::VecDeque, fmt, mem, os::raw::c_void};

use winapi::{
    shared::{
        dxgi,
        dxgi1_4,
        windef::{HWND, RECT},
        winerror,
    },
    um::{
        synchapi,
        winbase,
        winnt::HANDLE,
        winuser::GetClientRect,
    },
};

use crate::{conv, resource as r, Backend, Device, Instance, PhysicalDevice, QueueFamily};
use hal::{self, device::{Device as _}, format as f, image as i, window as w};

impl Instance {
    pub fn create_surface_from_hwnd(&self, hwnd: *mut c_void) -> Surface {
        Surface {
            factory: self.factory,
            wnd_handle: hwnd as *mut _,
            presentation: None,
        }
    }
}

#[derive(Debug)]
struct Presentation {
    swapchain: Swapchain,
    format: f::Format,
    size: w::Extent2D,
}

pub struct Surface {
    pub(crate) factory: native::WeakPtr<dxgi1_4::IDXGIFactory4>,
    pub(crate) wnd_handle: HWND,
    presentation: Option<Presentation>,
}

impl fmt::Debug for Surface {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("Surface")
    }
}

unsafe impl Send for Surface {}
unsafe impl Sync for Surface {}

impl Surface {
    pub(crate) unsafe fn present(&self) {
        self.presentation
            .as_ref()
            .unwrap()
            .swapchain
            .inner
            .Present(1, 0);
    }
}

impl w::Surface<Backend> for Surface {
    fn supports_queue_family(&self, queue_family: &QueueFamily) -> bool {
        match queue_family {
            &QueueFamily::Present => true,
            _ => false,
        }
    }

    fn capabilities(&self, _physical_device: &PhysicalDevice) -> w::SurfaceCapabilities {
        let current_extent = unsafe {
            let mut rect: RECT = mem::zeroed();
            if GetClientRect(self.wnd_handle as *mut _, &mut rect as *mut RECT) == 0 {
                panic!("GetClientRect failed");
            }
            Some(w::Extent2D {
                width: (rect.right - rect.left) as u32,
                height: (rect.bottom - rect.top) as u32,
            })
        };

        w::SurfaceCapabilities {
            present_modes: w::PresentMode::FIFO,                  //TODO
            composite_alpha_modes: w::CompositeAlphaMode::OPAQUE, //TODO
            image_count: 2 ..= dxgi::DXGI_MAX_SWAP_CHAIN_BUFFERS,
            current_extent,
            extents: w::Extent2D {
                width: 16,
                height: 16,
            } ..= w::Extent2D {
                width: 4096,
                height: 4096,
            },
            max_image_layers: 1,
            usage: i::Usage::COLOR_ATTACHMENT | i::Usage::TRANSFER_SRC | i::Usage::TRANSFER_DST,
        }
    }

    fn supported_formats(&self, _physical_device: &PhysicalDevice) -> Option<Vec<f::Format>> {
        Some(vec![
            f::Format::Bgra8Srgb,
            f::Format::Bgra8Unorm,
            f::Format::Rgba8Srgb,
            f::Format::Rgba8Unorm,
            f::Format::A2b10g10r10Unorm,
            f::Format::Rgba16Sfloat,
        ])
    }
}

impl w::PresentationSurface<Backend> for Surface {
    type SwapchainImage = r::ImageView;

    unsafe fn configure_swapchain(
        &mut self,
        device: &Device,
        config: w::SwapchainConfig,
    ) -> Result<(), w::CreationError> {
        assert!(i::Usage::COLOR_ATTACHMENT.contains(config.image_usage));

        let swapchain = match self.presentation.take() {
            Some(present) => {
                if present.format == config.format && present.size == config.extent {
                    self.presentation = Some(present);
                    return Ok(());
                }
                // can't have image resources in flight used by GPU
                device.wait_idle().unwrap();

                let inner = present.swapchain.release_resources();
                let result = inner.ResizeBuffers(
                    config.image_count,
                    config.extent.width,
                    config.extent.height,
                    conv::map_format_nosrgb(config.format).unwrap(),
                    dxgi::DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
                );
                if result != winerror::S_OK {
                    error!("ResizeBuffers failed with 0x{:x}", result as u32);
                    return Err(w::CreationError::WindowInUse(hal::device::WindowInUse));
                }
                inner
            }
            None => {
                let (swapchain, _) =
                    device.create_swapchain_impl(&config, self.wnd_handle, self.factory.clone())?;
                swapchain
            }
        };

        self.presentation = Some(Presentation {
            swapchain: device.wrap_swapchain(swapchain, &config),
            format: config.format,
            size: config.extent,
        });
        Ok(())
    }

    unsafe fn unconfigure_swapchain(&mut self, device: &Device) {
        if let Some(mut present) = self.presentation.take() {
            let _ = present.swapchain.wait(winbase::INFINITE);
            let _ = device.wait_idle(); //TODO: this shouldn't be needed,
            // but it complains that the queue is still used otherwise
            device.destroy_swapchain(present.swapchain);
        }
    }

    unsafe fn acquire_image(
        &mut self,
        timeout_ns: u64,
    ) -> Result<(r::ImageView, Option<w::Suboptimal>), w::AcquireError> {
        let present = self.presentation.as_mut().unwrap();
        let sc = &mut present.swapchain;
 
        sc.wait((timeout_ns / 1_000_000) as u32)?;

        let index = sc.inner.GetCurrentBackBufferIndex();
        let view = r::ImageView {
            resource: sc.resources[index as usize],
            handle_srv: None,
            handle_rtv: Some(sc.rtv_heap.at(index as _, 0).cpu),
            handle_uav: None,
            handle_dsv: None,
            dxgi_format: conv::map_format(present.format).unwrap(),
            num_levels: 1,
            mip_levels: (0, 1),
            layers: (0, 1),
            kind: i::Kind::D2(present.size.width, present.size.height, 1, 1),
        };

        Ok((view, None))
    }
}

#[derive(Debug)]
pub struct Swapchain {
    pub(crate) inner: native::WeakPtr<dxgi1_4::IDXGISwapChain3>,
    pub(crate) frame_queue: VecDeque<usize>,
    #[allow(dead_code)]
    pub(crate) rtv_heap: r::DescriptorHeap,
    // need to associate raw image pointers with the swapchain so they can be properly released
    // when the swapchain is destroyed
    pub(crate) resources: Vec<native::Resource>,
    pub(crate) waitable: HANDLE,
}

impl Swapchain {
    pub(crate) unsafe fn release_resources(self) -> native::WeakPtr<dxgi1_4::IDXGISwapChain3> {
        for resource in &self.resources {
            resource.destroy();
        }
        self.rtv_heap.destroy();
        self.inner
    }

    pub(crate) fn wait(&mut self, timeout_ms: u32) -> Result<(), w::AcquireError> {
        match unsafe {
            synchapi::WaitForSingleObject(self.waitable, timeout_ms)
        } {
            winbase::WAIT_ABANDONED |
            winbase::WAIT_FAILED => Err(w::AcquireError::DeviceLost(hal::device::DeviceLost)),
            winbase::WAIT_OBJECT_0 => Ok(()),
            winerror::WAIT_TIMEOUT => Err(w::AcquireError::Timeout),
            hr => panic!("Unexpected wait status 0x{:X}", hr),
        }
    }
}

impl w::Swapchain<Backend> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        _timout_ns: u64,
        _semaphore: Option<&r::Semaphore>,
        _fence: Option<&r::Fence>,
    ) -> Result<(w::SwapImageIndex, Option<w::Suboptimal>), w::AcquireError> {
        // TODO: sync

        let index = self.inner.GetCurrentBackBufferIndex();
        if false {
            // TODO: we need to block this at some point? (running out of backbuffers)
            //let num_images = self.images.len();
            self.frame_queue.push_back(index as usize);
        }

        // TODO:
        Ok((index, None))
    }
}

unsafe impl Send for Swapchain {}
unsafe impl Sync for Swapchain {}
