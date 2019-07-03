use hal::window::Extent2D;
use hal::{self, format as f, image, CompositeAlpha};

use crate::{native, Backend, GlContainer, PhysicalDevice, QueueFamily};
use hal::format::Format;

use std::{
    ffi::{CString, OsStr},
    mem,
    os::{raw::c_void, windows::ffi::OsStrExt},
    ptr,
};
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::*;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

pub mod wgl_sys {
    include!(concat!(env!("OUT_DIR"), "/wgl_sys.rs"));
}

pub mod wgl_ext_sys {
    include!(concat!(env!("OUT_DIR"), "/wgl_ext_sys.rs"));
}

#[link(name = "opengl32")]
extern "C" {}

#[cfg(feature = "winit")]
use winit;

pub(crate) struct Entry {
    hwnd: HWND,
    pub(crate) hdc: HDC,
    pub(crate) wgl: wgl_ext_sys::Wgl,
    lib: HMODULE,
}

unsafe impl Send for Entry {}
unsafe impl Sync for Entry {}

impl Entry {
    pub fn new() -> Self {
        unsafe {
            let mut class: WNDCLASSEXW = mem::zeroed();
            let instance = GetModuleHandleW(ptr::null());
            let class_name = OsStr::new("gfx-rs wgl")
                .encode_wide()
                .chain(Some(0).into_iter())
                .collect::<Vec<_>>();

            class.cbSize = mem::size_of::<WNDCLASSEXW>() as UINT;
            class.lpszClassName = class_name.as_ptr();
            class.lpfnWndProc = Some(DefWindowProcW);

            RegisterClassExW(&class);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                std::ptr::null(),
                0,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null_mut(),
            );

            let hdc = GetDC(hwnd);

            let desc = PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_SUPPORT_OPENGL,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 8,
                cRedBits: 0,
                cRedShift: 0,
                cGreenBits: 0,
                cGreenShift: 0,
                cBlueBits: 0,
                cBlueShift: 0,
                cAlphaBits: 8,
                cAlphaShift: 0,
                cAccumBits: 0,
                cAccumRedBits: 0,
                cAccumGreenBits: 0,
                cAccumBlueBits: 0,
                cAccumAlphaBits: 0,
                cDepthBits: 0,
                cStencilBits: 0,
                cAuxBuffers: 0,
                iLayerType: PFD_MAIN_PLANE,
                bReserved: 0,
                dwLayerMask: 0,
                dwVisibleMask: 0,
                dwDamageMask: 0,
            };

            let format_id = ChoosePixelFormat(hdc, &desc);
            SetPixelFormat(hdc, format_id, &desc);
            let hglrc = wglCreateContext(hdc);

            println!("{:?}", (hwnd, hdc, format_id, hglrc));

            wglMakeCurrent(hdc, hglrc);

            let name = OsStr::new("opengl32.dll")
                .encode_wide()
                .chain(Some(0).into_iter())
                .collect::<Vec<_>>();

            let lib = LoadLibraryW(name.as_ptr());

            let wgl = wgl_ext_sys::Wgl::load_with(|sym| {
                let sym = CString::new(sym.as_bytes()).unwrap();
                let addr = wgl_sys::GetProcAddress(sym.as_ptr()) as *const ();
                if !addr.is_null() {
                    addr as *const _
                } else {
                    GetProcAddress(lib, sym.as_ptr()) as *const _
                }
            });

            Entry {
                hwnd,
                hdc: hdc as _,
                wgl,
                lib,
            }
        }
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
        }
    }
}

lazy_static! {
    // Entry function pointers
    pub(crate) static ref WGL_ENTRY: Entry = Entry::new();
}

pub struct Instance {
    pub(crate) ctxt: DeviceContext,
}

impl Instance {
    pub fn create(_name: &str, version: u32) -> Self {
        unsafe {
            let glrc = WGL_ENTRY.wgl.CreateContextAttribsARB(
                WGL_ENTRY.hdc as *const _,
                ptr::null(),
                ptr::null(),
            ) as HGLRC;

            wglMakeCurrent(WGL_ENTRY.hdc as *mut _, glrc);

            Instance {
                ctxt: DeviceContext {
                    ctxt: Context { glrc },
                    hdc: WGL_ENTRY.hdc,
                },
            }
        }
    }

    #[cfg(windows)]
    pub fn create_surface_from_hwnd(&self, hwnd: *mut c_void) -> Surface {
        Surface {
            hwnd: hwnd as *mut _,
        }
    }

    #[cfg(feature = "winit")]
    pub fn create_surface(&self, window: &winit::Window) -> Surface {
        use winit::os::windows::WindowExt;

        let hwnd = window.get_hwnd();
        self.create_surface_from_hwnd(hwnd as *mut _)
    }
}

impl hal::Instance for Instance {
    type Backend = Backend;

    fn enumerate_adapters(&self) -> Vec<hal::Adapter<Backend>> {
        let gl_container = GlContainer::from_fn_proc(|s| unsafe {
            let sym = CString::new(s.as_bytes()).unwrap();
            let addr = wgl_sys::GetProcAddress(sym.as_ptr()) as *const ();
            if !addr.is_null() {
                addr as *const _
            } else {
                GetProcAddress(WGL_ENTRY.lib, sym.as_ptr()) as *const _
            }
        });
        let adapter = PhysicalDevice::new_adapter(self.ctxt, gl_container);
        vec![adapter]
    }
}

#[derive(Debug)]
pub struct Surface {
    pub(crate) hwnd: HWND,
}

// TODO: high -msiglreith
unsafe impl Send for Surface {}
unsafe impl Sync for Surface {}

impl Surface {
    fn get_extent(&self) -> hal::window::Extent2D {
        let mut rect: RECT = unsafe { mem::uninitialized() };
        unsafe {
            GetClientRect(self.hwnd, &mut rect);
        }
        hal::window::Extent2D {
            width: (rect.right - rect.left) as _,
            height: (rect.bottom - rect.top) as _,
        }
    }
}

impl hal::Surface<Backend> for Surface {
    fn kind(&self) -> hal::image::Kind {
        unimplemented!()
    }

    fn compatibility(
        &self,
        physical_device: &PhysicalDevice,
    ) -> (
        hal::SurfaceCapabilities,
        Option<Vec<Format>>,
        Vec<hal::PresentMode>,
    ) {
        let extent = self.get_extent();

        let caps = hal::SurfaceCapabilities {
            image_count: 2 .. 3,
            current_extent: Some(extent),
            extents: extent .. hal::window::Extent2D {
                width: extent.width + 1,
                height: extent.height + 1,
            },
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT | image::Usage::TRANSFER_SRC,
            composite_alpha: CompositeAlpha::OPAQUE, //TODO
        };
        let present_modes = vec![
            hal::PresentMode::Fifo, //TODO
        ];

        (
            caps,
            Some(vec![f::Format::Rgba8Srgb, f::Format::Bgra8Srgb]),
            present_modes,
        )
    }

    fn supports_queue_family(&self, _queue_family: &QueueFamily) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct Swapchain {
    pub(crate) fbos: Vec<native::FrameBuffer>,
    pub(crate) context: PresentContext,
    pub(crate) extent: Extent2D,
}
impl Swapchain {
    pub(crate) fn make_current(&self) {
        self.context.make_current();
    }

    pub(crate) fn swap_buffers(&self) {
        self.context.swap_buffers();
    }
}

impl hal::Swapchain<Backend> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
        _semaphore: Option<&native::Semaphore>,
        _fence: Option<&native::Fence>,
    ) -> Result<(hal::SwapImageIndex, Option<hal::window::Suboptimal>), hal::AcquireError> {
        Ok((0, None)) // TODO
    }
}

/// Basic abstraction for wgl context handles.
#[derive(Debug, Copy, Clone)]
struct Context {
    glrc: HGLRC,
}

impl Context {
    unsafe fn make_current(&self, hdc: HDC) {
        wglMakeCurrent(hdc, self.glrc);
    }
}

/// Owned context for devices and instances.
#[derive(Debug, Copy, Clone)]
pub(crate) struct DeviceContext {
    /// Owned wgl context.
    ctxt: Context,

    /// Device context owned by the corresponding instance.
    ///
    /// This refers to either a pbuffer or dummy window. Therefore not used for actual presentation.
    hdc: HDC,
}

// TODO
unsafe impl Send for DeviceContext {}
unsafe impl Sync for DeviceContext {}

impl DeviceContext {
    pub(crate) fn make_current(&self) {
        unsafe {
            self.ctxt.make_current(self.hdc);
        }
    }
}

/// Owned context for swapchains which soley is required for presentation.
#[derive(Debug)]
pub(crate) struct PresentContext {
    /// Owned wgl context.
    ctxt: Context,

    /// Device context of the corresponding presentation surface.
    hdc: HDC,
}

// TODO
unsafe impl Send for PresentContext {}
unsafe impl Sync for PresentContext {}

impl PresentContext {
    pub(crate) fn new(surface: &Surface, device_ctxt: &DeviceContext) -> Self {
        // TODO: configuration options
        unsafe {
            let hdc = GetDC(surface.hwnd);

            let desc = PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 32,
                cRedBits: 0,
                cRedShift: 0,
                cGreenBits: 0,
                cGreenShift: 0,
                cBlueBits: 0,
                cBlueShift: 0,
                cAlphaBits: 8,
                cAlphaShift: 0,
                cAccumBits: 0,
                cAccumRedBits: 0,
                cAccumGreenBits: 0,
                cAccumBlueBits: 0,
                cAccumAlphaBits: 0,
                cDepthBits: 0,
                cStencilBits: 0,
                cAuxBuffers: 0,
                iLayerType: PFD_MAIN_PLANE,
                bReserved: 0,
                dwLayerMask: 0,
                dwVisibleMask: 0,
                dwDamageMask: 0,
            };

            let format_id = ChoosePixelFormat(hdc, &desc);
            SetPixelFormat(hdc, format_id, &desc);

            let glrc = WGL_ENTRY.wgl.CreateContextAttribsARB(
                hdc as *const _,
                device_ctxt.ctxt.glrc as _,
                ptr::null(),
            ) as HGLRC;

            wglMakeCurrent(hdc, glrc);

            PresentContext {
                ctxt: Context { glrc },
                hdc,
            }
        }
    }

    pub(crate) fn make_current(&self) {
        unsafe {
            self.ctxt.make_current(self.hdc);
        }
    }

    fn swap_buffers(&self) {
        unsafe {
            SwapBuffers(self.hdc);
        }
    }
}
