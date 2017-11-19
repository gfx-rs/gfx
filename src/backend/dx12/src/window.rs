use std::collections::VecDeque;
use std::{mem, ptr};

use dxguid;
#[cfg(feature = "winit")]
use winit;
use winapi;
use wio::com::ComPtr;

use hal::{self, format as f, image as i};
use {conv, native as n, Backend, Device, Instance, PhysicalDevice, QueueFamily};

use std::os::raw::c_void;

impl Instance {
    pub fn create_surface_from_hwnd(&self, hwnd: *mut c_void) -> Surface {
        let (width, height) = unsafe {
            use winapi::RECT;
            use user32::GetClientRect;
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
    factory: ComPtr<winapi::IDXGIFactory4>,
    wnd_handle: winapi::HWND,
    width: u32,
    height: u32,
}

impl hal::Surface<Backend> for Surface {
    fn supports_queue_family(&self, _queue_family: &QueueFamily) -> bool { true }
    fn get_kind(&self) -> i::Kind {
        let aa = i::AaMode::Single;
        i::Kind::D2(self.width as i::Size, self.height as i::Size, aa)
    }

    fn capabilities_and_formats(
        &self, _: &PhysicalDevice,
    ) -> (hal::SurfaceCapabilities, Vec<f::Format>) {
        use hal::format::ChannelType::*;
        use hal::format::SurfaceType::*;

        let extent = hal::window::Extent2d {
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
            f::Format(B8_G8_R8_A8, Srgb),
            f::Format(B8_G8_R8_A8, Unorm),
            f::Format(R8_G8_B8_A8, Srgb),
            f::Format(R8_G8_B8_A8, Unorm),
            f::Format(R10_G10_B10_A2, Unorm),
            f::Format(R16_G16_B16_A16, Float),
        ];

        (capabilities, formats)
    }

    fn build_swapchain<C>(
        &mut self,
        config: hal::SwapchainConfig,
        present_queue: &hal::CommandQueue<Backend, C>,
    ) -> (Swapchain, hal::Backbuffer<Backend>) {
        let mut swap_chain: *mut winapi::IDXGISwapChain1 = ptr::null_mut();
        let mut format = config.color_format;
        if format.1 == f::ChannelType::Srgb {
            // Apparently, swap chain doesn't like sRGB, but the RTV can still have some:
            // https://www.gamedev.net/forums/topic/670546-d3d12srgb-buffer-format-for-swap-chain/
            // [15716] DXGI ERROR: IDXGIFactory::CreateSwapchain: Flip model swapchains (DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL and DXGI_SWAP_EFFECT_FLIP_DISCARD) only support the following Formats: (DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM), assuming the underlying Device does as well.
            format.1 = f::ChannelType::Unorm;
        }
        let format = conv::map_format(format).unwrap(); // TODO: error handling
        let mut device = present_queue.as_raw().device.clone();

        let rtv_desc = winapi::D3D12_RENDER_TARGET_VIEW_DESC {
            Format: conv::map_format(config.color_format).unwrap(),
            ViewDimension: winapi::D3D12_RTV_DIMENSION_TEXTURE2D,
            .. unsafe { mem::zeroed() }
        };
        let rtv_heap = Device::create_descriptor_heap_impl(
            &mut device,
            winapi::D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            false,
            config.image_count as _,
        );

        // TODO: double-check values
        let desc = winapi::DXGI_SWAP_CHAIN_DESC1 {
            AlphaMode: winapi::DXGI_ALPHA_MODE_IGNORE,
            BufferCount: config.image_count,
            Width: self.width,
            Height: self.height,
            Format: format,
            Flags: 0,
            BufferUsage: winapi::DXGI_USAGE_RENDER_TARGET_OUTPUT,
            SampleDesc: winapi::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Scaling: winapi::DXGI_SCALING_STRETCH,
            Stereo: false as winapi::BOOL,
            SwapEffect: winapi::DXGI_SWAP_EFFECT(4), // TODO: DXGI_SWAP_EFFECT_FLIP_DISCARD missing in winapi
        };

        let hr = unsafe {
            self.factory.CreateSwapChainForHwnd(
                present_queue.as_raw().raw.as_mut() as *mut _ as *mut winapi::IUnknown,
                self.wnd_handle,
                &desc,
                ptr::null(),
                ptr::null_mut(),
                &mut swap_chain as *mut *mut _,
            )
        };

        if !winapi::SUCCEEDED(hr) {
            error!("error on swapchain creation 0x{:x}", hr);
        }

        let mut swap_chain = unsafe { ComPtr::<winapi::IDXGISwapChain3>::new(swap_chain as *mut winapi::IDXGISwapChain3) };

        // Get backbuffer images
        let images = (0 .. config.image_count).map(|i| {
            let mut resource: *mut winapi::ID3D12Resource = ptr::null_mut();
            unsafe {
                swap_chain.GetBuffer(
                    i as _,
                    &dxguid::IID_ID3D12Resource,
                    &mut resource as *mut *mut _ as *mut *mut _);
            }

            let rtv_handle = rtv_heap.at(i as _).cpu;
            unsafe {
                device.CreateRenderTargetView(resource, &rtv_desc, rtv_handle);
            }

            let kind = i::Kind::D2(self.width as u16, self.height as u16, 1.into());
            n::Image {
                resource,
                kind,
                usage: i::Usage::COLOR_ATTACHMENT,
                dxgi_format: format,
                bits_per_texel: config.color_format.0.describe_bits().total,
                num_levels: 1,
                num_layers: 1,
                clear_cv: Some(rtv_handle),
                clear_dv: None,
                clear_sv: None,
            }
        }).collect();

        let swapchain = Swapchain {
            inner: swap_chain,
            next_frame: 0,
            frame_queue: VecDeque::new(),
            rtv_heap,
        };

        (swapchain, hal::Backbuffer::Images(images))
    }
}

pub struct Swapchain {
    inner: ComPtr<winapi::IDXGISwapChain3>,
    next_frame: usize,
    frame_queue: VecDeque<usize>,
    #[allow(dead_code)]
    rtv_heap: n::DescriptorHeap,
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

    fn present<C>(
        &mut self,
        _: &mut hal::CommandQueue<Backend, C>,
        _wait_semaphores: &[&n::Semaphore],
    ) {
        // TODO: wait semaphores
        unsafe { self.inner.Present(1, 0); }
    }
}
