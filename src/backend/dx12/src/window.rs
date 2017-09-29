
use core::{self, image};
use dxguid;
use std::collections::VecDeque;
use std::ptr;
use winit;
use winapi;
use {conv, native as n, Adapter, Backend, ComPtr, Instance, QueueFamily};

use winit::os::windows::WindowExt;

impl Instance {
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        let (width, height) = window.get_inner_size_pixels().unwrap();
        Surface {
            factory: self.factory.clone(),
            wnd_handle: window.get_hwnd() as *mut _,
            width: width,
            height: height,
        }
    }
}

pub struct Surface {
    factory: ComPtr<winapi::IDXGIFactory4>,
    wnd_handle: winapi::HWND,
    width: u32,
    height: u32,
}

impl core::Surface<Backend> for Surface {
    fn supports_queue(&self, _queue_family: &QueueFamily) -> bool { true }
    fn get_kind(&self) -> core::image::Kind {
        use core::image::Size;

        let aa = core::image::AaMode::Single;
        core::image::Kind::D2(self.width as Size, self.height as Size, aa)
    }

    fn surface_capabilities(&self, _: &Adapter) -> core::SurfaceCapabilities {
        let extent = core::window::Extent2d {
            width: self.width,
            height: self.height,
        };

        core::SurfaceCapabilities {
            image_count: 2..16, // we currently use a flip effect which supports 2..16 buffers
            current_extent: Some(extent),
            extents: extent..extent,
            max_image_layers: 1,
        }
    }

    fn build_swapchain<C>(
        &mut self,
        config: core::SwapchainConfig,
        present_queue: &core::CommandQueue<Backend, C>,
    ) -> (Swapchain, Vec<core::Backbuffer<Backend>>) {
        let mut swap_chain: *mut winapi::IDXGISwapChain1 = ptr::null_mut();
        let buffer_count = 2; // TODO: user-defined value
        let mut format = config.color_format;
        if format.1 == core::format::ChannelType::Srgb {
            // Apparently, swap chain doesn't like sRGB, but the RTV can still have some:
            // https://www.gamedev.net/forums/topic/670546-d3d12srgb-buffer-format-for-swap-chain/
            // [15716] DXGI ERROR: IDXGIFactory::CreateSwapchain: Flip model swapchains (DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL and DXGI_SWAP_EFFECT_FLIP_DISCARD) only support the following Formats: (DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM), assuming the underlying Device does as well.
            format.1 = core::format::ChannelType::Unorm;
        }
        let format = conv::map_format(format, true).unwrap(); // TODO: error handling

        // TODO: double-check values
        let desc = winapi::DXGI_SWAP_CHAIN_DESC1 {
            AlphaMode: winapi::DXGI_ALPHA_MODE_IGNORE,
            BufferCount: buffer_count,
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
        let backbuffers = (0..buffer_count).map(|i| {
            let mut resource: *mut winapi::ID3D12Resource = ptr::null_mut();
            unsafe {
                swap_chain.GetBuffer(
                    i,
                    &dxguid::IID_ID3D12Resource,
                    &mut resource as *mut *mut _ as *mut *mut _);
            }

            let kind = image::Kind::D2(self.width as u16, self.height as u16, 1.into());
            let color = n::Image {
                resource,
                kind,
                dxgi_format: format,
                bits_per_texel: config.color_format.0.get_total_bits(),
                levels: 1,
                layers: 1,
            };

            core::Backbuffer {
                color,
                depth_stencil: None, // TODO
            }
        }).collect::<Vec<_>>();

        let swapchain = Swapchain {
            inner: swap_chain,
            next_frame: 0,
            frame_queue: VecDeque::new(),
        };

        (swapchain, backbuffers)
    }
}

pub struct Swapchain {
    inner: ComPtr<winapi::IDXGISwapChain3>,
    next_frame: usize,
    frame_queue: VecDeque<usize>,
}

impl core::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, _sync: core::FrameSync<Backend>) -> core::Frame {
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
        core::Frame::new(index as usize)
    }

    fn present<C>(
        &mut self,
        _: &mut core::CommandQueue<Backend, C>,
        _wait_semaphores: &[&n::Semaphore],
    ) {
        // TODO: wait semaphores
        unsafe { self.inner.Present(1, 0); }
    }
}
