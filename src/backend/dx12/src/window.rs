
use core::{self, image};
use dxguid;
use std::collections::VecDeque;
use std::ptr;
use winapi;
use wio::com::ComPtr;
use {data, native as n, Backend, QueueFamily};

pub struct Surface {
    factory: ComPtr<winapi::IDXGIFactory4>,
    wnd_handle: winapi::HWND,
    width: u32,
    height: u32,
}

impl core::Surface<Backend> for Surface {
    fn supports_queue(&self, _queue_family: &QueueFamily) -> bool { true }
    fn build_swapchain<C>(
        &mut self,
        config: core::SwapchainConfig,
        present_queue: &core::CommandQueue<Backend, C>,
    ) -> Swapchain {
        let mut swap_chain: *mut winapi::IDXGISwapChain1 = ptr::null_mut();
        let buffer_count = 2; // TODO: user-defined value
        let format = data::map_format(config.color_format, true).unwrap(); // TODO: error handling

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
            error!("error on swapchain creation {:x}", hr);
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
            };

            core::Backbuffer {
                color,
                depth_stencil: None, // TODO
            }
        }).collect::<Vec<_>>();

        Swapchain {
            inner: swap_chain,
            next_frame: 0,
            frame_queue: VecDeque::new(),
            images: backbuffers,
        }
    }
}

pub struct Swapchain {
    inner: ComPtr<winapi::IDXGISwapChain3>,
    next_frame: usize,
    frame_queue: VecDeque<usize>,
    images: Vec<core::Backbuffer<Backend>>,
}

impl core::Swapchain<Backend> for Swapchain {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<Backend>] {
        &self.images
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<Backend>) -> core::Frame {
        // TODO: sync
        // TODO: we need to block this at some point? (running out of backbuffers)
        // let num_images = self.images.len();
        // let index = self.next_frame;
        // self.frame_queue.push_back(index);
        // self.next_frame = (self.next_frame + 1) % num_images;
        // unsafe { core::Frame::new(index) };

        // TODO:
        let index = unsafe { self.inner.GetCurrentBackBufferIndex() };
        unsafe { core::Frame::new(index as usize) }
    }

    fn present<C>(
        &mut self,
        present_queue: &mut core::CommandQueue<Backend, C>,
        wait_semaphores: &[&n::Semaphore],
    ) {
        // TODO: wait semaphores
        unsafe { self.inner.Present(1, 0); }
    }
}
