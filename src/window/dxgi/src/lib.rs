#[deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate dxguid;
extern crate winapi;
extern crate winit;
extern crate gfx_core as core;
extern crate gfx_device_dx11 as device_dx11;
extern crate gfx_device_dx12 as device_dx12;
extern crate wio;

use std::ptr;
use std::rc::Rc;
use std::os::raw::c_void;
use std::collections::VecDeque;
use winit::os::windows::WindowExt;
use core::{handle as h, memory, texture as tex};
use wio::com::ComPtr;

/*
pub fn resize_swap_chain<Cf>(&mut self, factory: &mut Factory, width: Size, height: Size)
                            -> Result<h::RenderTargetView<Resources, Cf>, winapi::HRESULT>
where Cf: format::RenderFormat
{
    let result = unsafe {
        (*self.swap_chain).ResizeBuffers(0,
            width as winapi::UINT, height as winapi::UINT,
            winapi::DXGI_FORMAT_UNKNOWN, 0)
    };
    if result == winapi::S_OK {
        self.size = (width, height);
        let raw = self.make_back_buffer(factory);
        Ok(memory::Typed::new(raw))
    } else {
        Err(result)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum InitError {
    /// Unable to create a window.
    Window,
    /// Unable to map format to DXGI.
    Format(format::Format),
    /// Unable to find a supported driver type.
    DriverType,
}

/// Update the internal dimensions of the main framebuffer targets. Generic version over the format.
pub fn update_views<Cf>(window: &mut Window, factory: &mut Factory, width: u16, height: u16)
            -> Result<h::RenderTargetView<Resources, Cf>, f::TargetViewError>
where Cf: format::RenderFormat
{

    factory.cleanup();
    // device.clear_state();
    // device.cleanup();

    window.resize_swap_chain::<Cf>(factory, width, height)
        .map_err(|hr| {
            error!("Resize failed with code {:X}", hr);
            f::TargetViewError::NotDetached
        }
    )
}
*/

fn get_window_dimensions(window: &winit::Window) -> tex::Dimensions {
    let (width, height) = window.get_inner_size().unwrap();
    ((width as f32 * window.hidpi_factor()) as tex::Size, (height as f32 * window.hidpi_factor()) as tex::Size, 1, 1.into())
}

pub struct Surface11 {
    factory: ComPtr<winapi::IDXGIFactory2>,
    window: Rc<winit::Window>,
    manager: h::Manager<device_dx11::Resources>,
}

impl core::Surface<device_dx11::Backend> for Surface11 {
    type Swapchain = Swapchain11;

    fn supports_queue(&self, _: &device_dx11::QueueFamily) -> bool { true }
    fn build_swapchain<Q>(&mut self, config: core::SwapchainConfig, present_queue: &Q) -> Swapchain11
        where Q: AsRef<device_dx11::CommandQueue>
    {
        use core::handle::Producer;

        let present_queue = present_queue.as_ref();
        let dim = get_window_dimensions(&self.window);

        let mut swap_chain = {
            let mut swap_chain: *mut winapi::IDXGISwapChain1 = ptr::null_mut();
            let buffer_count = 2; // TODO: user-defined value

            // TODO: double-check values
            let desc = winapi::DXGI_SWAP_CHAIN_DESC1 {
                AlphaMode: winapi::DXGI_ALPHA_MODE(0),
                BufferCount: buffer_count,
                Width: dim.0 as u32,
                Height: dim.1 as u32,
                Format: device_dx11::data::map_format(config.color_format, true).unwrap(), // TODO: error handling
                Flags: 0,
                BufferUsage: winapi::DXGI_USAGE_RENDER_TARGET_OUTPUT,
                SampleDesc: winapi::DXGI_SAMPLE_DESC { // TODO
                    Count: 1,
                    Quality: 0,
                },
                Scaling: winapi::DXGI_SCALING(0),
                Stereo: false as winapi::BOOL,
                SwapEffect: winapi::DXGI_SWAP_EFFECT(4), // TODO: FLIP_DISCARD
            };

            let hr = unsafe {
                self.factory.as_mut().CreateSwapChainForHwnd(
                    present_queue.device.as_mut() as *mut _ as *mut winapi::IUnknown,
                    self.window.get_hwnd() as *mut _,
                    &desc,
                    ptr::null(),
                    ptr::null_mut(),
                    &mut swap_chain as *mut *mut _,
                )
            };

            if !winapi::SUCCEEDED(hr) {
                error!("error on swapchain creation {:x}", hr);
            }

            unsafe { ComPtr::new(swap_chain) }
        };

        let backbuffer = {
            let mut back_buffer: *mut winapi::ID3D11Texture2D = ptr::null_mut();
            unsafe {
                swap_chain.GetBuffer(
                    0,
                    &dxguid::ID3D11Texture2D::uuidof(),
                    &mut back_buffer as *mut *mut winapi::ID3D11Texture2D as *mut *mut _);
            }

            let kind = tex::Kind::D2(dim.0, dim.1, dim.3);
            let raw_tex = device_dx11::Texture::new(device_dx11::native::Texture::D2(back_buffer));
            let color_tex = self.manager.make_texture(
                                raw_tex,
                                tex::Info {
                                    kind,
                                    levels: 1,
                                    format: config.color_format.0,
                                    bind: memory::RENDER_TARGET,
                                    usage: memory::Usage::Data,
                                });

            let ds_tex = config.depth_stencil_format.map(|ds_format| {
                let info = tex::Info {
                    kind: tex::Kind::D2(dim.0, dim.1, dim.3),
                    levels: 1,
                    format: ds_format.0,
                    bind: memory::DEPTH_STENCIL,
                    usage: memory::Usage::Data,
                };

                let (usage, cpu_access) = device_dx11::data::map_usage(info.usage, info.bind);

                let desc = winapi::D3D11_TEXTURE2D_DESC {
                    Width: dim.0 as winapi::UINT,
                    Height: dim.1 as winapi::UINT,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: device_dx11::data::map_surface(info.format).unwrap(),
                    SampleDesc: device_dx11::data::map_anti_alias(dim.3),
                    Usage: usage,
                    BindFlags: device_dx11::data::map_bind(info.bind).0,
                    CPUAccessFlags: cpu_access.0,
                    MiscFlags: 0,
                };

                let mut raw = ptr::null_mut();
                let hr = unsafe {
                    present_queue.device.as_mut().CreateTexture2D(&desc, ptr::null(), &mut raw)
                };

                if !winapi::SUCCEEDED(hr) {
                    error!("DS texture creation failed on {:#?} with error {:x}", desc, hr);
                }

                self.manager.make_texture(
                    device_dx11::Texture::new(device_dx11::native::Texture::D2(raw)),
                    tex::Info {
                        kind: tex::Kind::D2(dim.0, dim.1, dim.3),
                        levels: 1,
                        format: ds_format.0,
                        bind: memory::DEPTH_STENCIL,
                        usage: memory::Usage::Data,
                    })
            });

            (color_tex, ds_tex)

        };

        Swapchain11 {
            swap_chain,
            images: [backbuffer],
        }
    }
}

pub struct Swapchain11 {
    swap_chain: ComPtr<winapi::IDXGISwapChain1>,
    images: [core::Backbuffer<device_dx11::Backend>; 1],
}

impl core::Swapchain<device_dx11::Backend> for Swapchain11 {
    fn get_backbuffers(&mut self) -> &[core::Backbuffer<device_dx11::Backend>] {
        &self.images
    }

    fn acquire_frame(&mut self, sync: core::FrameSync<device_dx11::Resources>) -> Result<core::Frame, ()> {
        // TODO: sync
        Ok(core::Frame::new(0))
    }

    fn present<Q>(&mut self, _present_queue: &mut Q, wait_semaphores: &[&h::Semaphore<device_dx11::Resources>])
        where Q: AsMut<device_dx11::CommandQueue>
    {
        // TODO: wait semaphores
        unsafe { self.swap_chain.Present(1, 0); }
    }
}

pub struct Window(Rc<winit::Window>);

impl Window {
    /// Create a new window.
    pub fn new(window: winit::Window) -> Self {
        Window(Rc::new(window))
    }

    /// Get internal winit window.
    pub fn raw(&self) -> &winit::Window {
        &self.0
    }
}

impl core::WindowExt<device_dx11::Backend> for Window {
    type Surface = Surface11;
    type Adapter = device_dx11::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface11, Vec<device_dx11::Adapter>) {
        let mut instance = device_dx11::Instance::create();
        let adapters = instance.enumerate_adapters();
        let surface = {
            Surface11 {
                factory: instance.0,
                window: self.0.clone(),
                manager: h::Manager::new()
            }
        };

        (surface, adapters)
    }
}

/*
impl core::WindowExt<device_dx12::Backend> for Window {
    type Surface = Surface12;
    type Adapter = device_dx12::Adapter;

    fn get_surface_and_adapters(&mut self) -> (Surface12, Vec<device_dx12::Adapter>) {
        let mut instance = device_dx12::Instance::create();
        let adapters = instance.enumerate_adapters();
        let surface = {
            let (width, height) = self.0.get_inner_size().unwrap();
            Surface12 {
                factory: instance.factory.clone(),
                wnd_handle: self.0.get_hwnd() as *mut _,
                manager: h::Manager::new(),
                width: width,
                height: height,
            }
        };

        (surface, adapters)
    }
}
*/
