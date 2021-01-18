//! EGL-based surface and swapchain.

use crate::{conv, native, GlContainer, PhysicalDevice, Starc};
use glow::HasContext;
use hal::{image, window as w};
use parking_lot::Mutex;
use std::{os::raw, ptr};

#[derive(Debug)]
pub struct Swapchain {
    framebuffer: glow::Framebuffer,
    renderbuffer: glow::Renderbuffer,
    /// Extent because the window lies
    extent: w::Extent2D,
    format: native::TextureFormat,
    channel: hal::format::ChannelType,
}

#[derive(Debug)]
pub struct Instance {
    wsi_library: Option<libloading::Library>,
    inner: Mutex<Inner>,
}

#[derive(Debug)]
pub struct Inner {
    egl: Starc<egl::DynamicInstance<egl::EGL1_4>>,
    version: (i32, i32),
    supports_native_window: bool,
    display: egl::Display,
    config: egl::Config,
    context: egl::Context,
    pbuffer: egl::Surface,
    wl_display: Option<*mut raw::c_void>,
}

unsafe impl Send for Instance {}
unsafe impl Sync for Instance {}

const EGL_PLATFORM_WAYLAND_KHR: u32 = 0x31D8;
const EGL_PLATFORM_X11_KHR: u32 = 0x31D5;

type XOpenDisplayFun =
    unsafe extern "system" fn(display_name: *const raw::c_char) -> *mut raw::c_void;

type WlDisplayConnectFun =
    unsafe extern "system" fn(display_name: *const raw::c_char) -> *mut raw::c_void;

type WlDisplayDisconnectFun = unsafe extern "system" fn(display: *const raw::c_void);

#[cfg(not(target_os = "android"))]
type WlEglWindowCreateFun = unsafe extern "system" fn(
    surface: *const raw::c_void,
    width: raw::c_int,
    height: raw::c_int,
) -> *mut raw::c_void;

type WlEglWindowResizeFun = unsafe extern "system" fn(
    window: *const raw::c_void,
    width: raw::c_int,
    height: raw::c_int,
    dx: raw::c_int,
    dy: raw::c_int,
);

type WlEglWindowDestroyFun = unsafe extern "system" fn(window: *const raw::c_void);

fn open_x_display() -> Option<(ptr::NonNull<raw::c_void>, libloading::Library)> {
    log::info!("Loading X11 library to get the current display");
    let library = libloading::Library::new("libX11.so").ok()?;
    let func: libloading::Symbol<XOpenDisplayFun> =
        unsafe { library.get(b"XOpenDisplay").unwrap() };
    let result = unsafe { func(ptr::null()) };
    ptr::NonNull::new(result).map(|ptr| (ptr, library))
}

fn test_wayland_display() -> Option<libloading::Library> {
    /* We try to connect and disconnect here to simply ensure there
     * is an active wayland display available.
     */
    log::info!("Loading Wayland library to get the current display");
    let client_library = libloading::Library::new("libwayland-client.so").ok()?;
    let wl_display_connect: libloading::Symbol<WlDisplayConnectFun> =
        unsafe { client_library.get(b"wl_display_connect").unwrap() };
    let wl_display_disconnect: libloading::Symbol<WlDisplayDisconnectFun> =
        unsafe { client_library.get(b"wl_display_disconnect").unwrap() };
    let display = ptr::NonNull::new(unsafe { wl_display_connect(ptr::null()) })?;
    unsafe { wl_display_disconnect(display.as_ptr()) };
    let library = libloading::Library::new("libwayland-egl.so").ok()?;
    Some(library)
}

/// Choose GLES framebuffer configuration.
fn choose_config(
    egl: &egl::DynamicInstance<egl::EGL1_4>,
    display: egl::Display,
) -> Result<(egl::Config, bool), hal::UnsupportedBackend> {
    //TODO: EGL_SLOW_CONFIG
    let tiers = [
        (
            "off-screen",
            &[egl::RENDERABLE_TYPE, egl::OPENGL_ES2_BIT][..],
        ),
        ("presentation", &[egl::SURFACE_TYPE, egl::WINDOW_BIT]),
        #[cfg(not(target_os = "android"))]
        ("native-render", &[egl::NATIVE_RENDERABLE, egl::TRUE as _]),
    ];

    let mut attributes = Vec::with_capacity(7);
    for tier_max in (0..tiers.len()).rev() {
        let name = tiers[tier_max].0;
        log::info!("Trying {}", name);

        attributes.clear();
        for &(_, tier_attr) in tiers[..=tier_max].iter() {
            attributes.extend_from_slice(tier_attr);
        }
        attributes.push(egl::NONE);

        match egl.choose_first_config(display, &attributes) {
            Ok(Some(config)) => {
                return Ok((config, tier_max >= 1));
            }
            Ok(None) => {
                log::warn!("No config found!");
            }
            Err(e) => {
                log::error!("error in choose_first_config: {:?}", e);
            }
        }
    }

    Err(hal::UnsupportedBackend)
}

impl Inner {
    fn create(
        egl: Starc<egl::DynamicInstance<egl::EGL1_4>>,
        display: egl::Display,
        wsi_library: Option<&libloading::Library>,
    ) -> Result<Self, hal::UnsupportedBackend> {
        let version = egl
            .initialize(display)
            .map_err(|_| hal::UnsupportedBackend)?;
        let vendor = egl.query_string(Some(display), egl::VENDOR).unwrap();
        let display_extensions = egl
            .query_string(Some(display), egl::EXTENSIONS)
            .unwrap()
            .to_string_lossy();
        log::info!(
            "Display vendor {:?}, version {:?}, extensions: {:?}",
            vendor,
            version,
            display_extensions
        );

        if log::max_level() >= log::LevelFilter::Trace {
            log::trace!("Configurations:");
            let config_count = egl.get_config_count(display).unwrap();
            let mut configurations = Vec::with_capacity(config_count);
            egl.get_configs(display, &mut configurations).unwrap();
            for &config in configurations.iter() {
                log::trace!("\tCONFORMANT=0x{:X}, RENDERABLE=0x{:X}, NATIVE_RENDERABLE=0x{:X}, SURFACE_TYPE=0x{:X}",
                    egl.get_config_attrib(display, config, egl::CONFORMANT).unwrap(),
                    egl.get_config_attrib(display, config, egl::RENDERABLE_TYPE).unwrap(),
                    egl.get_config_attrib(display, config, egl::NATIVE_RENDERABLE).unwrap(),
                    egl.get_config_attrib(display, config, egl::SURFACE_TYPE).unwrap(),
                );
            }
        }

        let (config, supports_native_window) = choose_config(&egl, display)?;
        egl.bind_api(egl::OPENGL_ES_API).unwrap();

        //TODO: make it so `Device` == EGL Context
        let mut context_attributes = vec![
            egl::CONTEXT_CLIENT_VERSION,
            3, // Request GLES 3.0 or higher
        ];
        if cfg!(debug_assertions) && wsi_library.is_none() && !cfg!(target_os = "android") {
            //TODO: figure out why this is needed
            context_attributes.push(egl::CONTEXT_OPENGL_DEBUG);
            context_attributes.push(egl::TRUE as _);
        }
        context_attributes.push(egl::NONE as _);
        let context = match egl.create_context(display, config, None, &context_attributes) {
            Ok(context) => context,
            Err(e) => {
                log::warn!("unable to create GLES 3.x context: {:?}", e);
                return Err(hal::UnsupportedBackend);
            }
        };

        let pbuffer = {
            let attributes = [egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE];
            egl.create_pbuffer_surface(display, config, &attributes)
                .map_err(|e| {
                    log::warn!("Error in create_pbuffer_surface: {:?}", e);
                    hal::UnsupportedBackend
                })?
        };

        Ok(Self {
            egl,
            display,
            version,
            supports_native_window,
            config,
            context,
            pbuffer,
            wl_display: None,
        })
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Err(e) = self.egl.destroy_context(self.display, self.context) {
            log::warn!("Error in destroy_context: {:?}", e);
        }
        if let Err(e) = self.egl.terminate(self.display) {
            log::warn!("Error in terminate: {:?}", e);
        }
    }
}

impl hal::Instance<crate::Backend> for Instance {
    fn create(_: &str, _: u32) -> Result<Self, hal::UnsupportedBackend> {
        let egl = match unsafe { egl::DynamicInstance::<egl::EGL1_4>::load_required() } {
            Ok(egl) => Starc::new(egl),
            Err(e) => {
                log::warn!("Unable to open libEGL.so: {:?}", e);
                return Err(hal::UnsupportedBackend);
            }
        };

        let client_extensions = egl.query_string(None, egl::EXTENSIONS);

        let client_ext_str = match client_extensions {
            Ok(ext) => ext.to_string_lossy().into_owned(),
            Err(_) => String::new(),
        };
        log::info!("Client extensions: {:?}", client_ext_str);

        let mut wsi_library = None;

        let wayland_library = if client_ext_str.contains(&"EGL_EXT_platform_wayland") {
            test_wayland_display()
        } else {
            None
        };

        let x11_display_library = if client_ext_str.contains(&"EGL_EXT_platform_x11") {
            open_x_display()
        } else {
            None
        };

        let display = if let (Some(library), Some(egl)) =
            (wayland_library, egl.upcast::<egl::EGL1_5>())
        {
            log::info!("Using Wayland platform");
            let display_attributes = [egl::ATTRIB_NONE];
            wsi_library = Some(library);
            egl.get_platform_display(
                EGL_PLATFORM_WAYLAND_KHR,
                egl::DEFAULT_DISPLAY,
                &display_attributes,
            )
            .unwrap()
        } else if let (Some((display, library)), Some(egl)) =
            (x11_display_library, egl.upcast::<egl::EGL1_5>())
        {
            log::info!("Using X11 platform");
            let display_attributes = [egl::ATTRIB_NONE];
            wsi_library = Some(library);
            egl.get_platform_display(EGL_PLATFORM_X11_KHR, display.as_ptr(), &display_attributes)
                .unwrap()
        } else {
            log::info!("Using default platform");
            egl.get_display(egl::DEFAULT_DISPLAY).unwrap()
        };

        let inner = Inner::create(egl.clone(), display, wsi_library.as_ref())?;

        Ok(Instance {
            inner: Mutex::new(inner),
            wsi_library,
        })
    }

    fn enumerate_adapters(&self) -> Vec<hal::adapter::Adapter<crate::Backend>> {
        let inner = self.inner.lock();
        inner
            .egl
            .make_current(
                inner.display,
                Some(inner.pbuffer),
                Some(inner.pbuffer),
                Some(inner.context),
            )
            .unwrap();

        let context = unsafe {
            glow::Context::from_loader_function(|name| {
                inner
                    .egl
                    .get_proc_address(name)
                    .map_or(ptr::null(), |p| p as *const _)
            })
        };
        // Create physical device
        vec![PhysicalDevice::new_adapter(context)]
    }

    unsafe fn create_surface(
        &self,
        has_handle: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, w::InitError> {
        use raw_window_handle::RawWindowHandle as Rwh;

        let mut inner = self.inner.lock();
        #[cfg(not(target_os = "android"))]
        let (mut temp_xlib_handle, mut temp_xcb_handle);
        let native_window_ptr = match has_handle.raw_window_handle() {
            #[cfg(not(target_os = "android"))]
            Rwh::Xlib(handle) => {
                temp_xlib_handle = handle.window;
                &mut temp_xlib_handle as *mut _ as *mut std::ffi::c_void
            }
            #[cfg(not(target_os = "android"))]
            Rwh::Xcb(handle) => {
                temp_xcb_handle = handle.window;
                &mut temp_xcb_handle as *mut _ as *mut std::ffi::c_void
            }
            #[cfg(target_os = "android")]
            Rwh::Android(handle) => handle.a_native_window as *mut _ as *mut std::ffi::c_void,
            #[cfg(not(target_os = "android"))]
            Rwh::Wayland(handle) => {
                /* Wayland displays are not sharable between surfaces so if the
                 * surface we receive from this handle is from a different
                 * display, we must re-initialize the context.
                 *
                 * See gfx-rs/gfx#3545
                 */
                if inner
                    .wl_display
                    .map(|ptr| ptr != handle.display)
                    .unwrap_or(true)
                {
                    use std::ops::DerefMut;
                    let display_attributes = [egl::ATTRIB_NONE];
                    let display = inner
                        .egl
                        .upcast::<egl::EGL1_5>()
                        .unwrap()
                        .get_platform_display(
                            EGL_PLATFORM_WAYLAND_KHR,
                            handle.display,
                            &display_attributes,
                        )
                        .unwrap();

                    let new_inner =
                        Inner::create(inner.egl.clone(), display, self.wsi_library.as_ref())
                            .map_err(|_| w::InitError::UnsupportedWindowHandle)?;

                    let old_inner = std::mem::replace(inner.deref_mut(), new_inner);
                    inner.wl_display = Some(handle.display);
                    drop(old_inner);
                }

                let window = {
                    let wl_egl_window_create: libloading::Symbol<WlEglWindowCreateFun> = self
                        .wsi_library
                        .as_ref()
                        .expect("unsupported window")
                        .get(b"wl_egl_window_create")
                        .unwrap();
                    let result = wl_egl_window_create(handle.surface, 640, 480);
                    ptr::NonNull::new(result)
                };
                window.expect("unsupported window").as_ptr() as *mut _ as *mut std::ffi::c_void
            }
            other => panic!("Unsupported window: {:?}", other),
        };

        let mut attributes = vec![
            egl::RENDER_BUFFER as usize,
            if cfg!(target_os = "android") {
                egl::BACK_BUFFER as usize
            } else {
                egl::SINGLE_BUFFER as usize
            },
        ];
        if inner.version >= (1, 5) {
            // Always enable sRGB in EGL 1.5
            attributes.push(egl::GL_COLORSPACE as usize);
            attributes.push(egl::GL_COLORSPACE_SRGB as usize);
        }
        attributes.push(egl::ATTRIB_NONE);

        let raw = if let Some(egl) = inner.egl.upcast::<egl::EGL1_5>() {
            egl.create_platform_window_surface(
                inner.display,
                inner.config,
                native_window_ptr,
                &attributes,
            )
            .map_err(|e| {
                log::warn!("Error in create_platform_window_surface: {:?}", e);
                w::InitError::UnsupportedWindowHandle
            })
        } else {
            let attributes_i32: Vec<i32> = attributes.iter().map(|a| (*a as i32).into()).collect();
            inner
                .egl
                .create_window_surface(
                    inner.display,
                    inner.config,
                    native_window_ptr,
                    Some(&attributes_i32),
                )
                .map_err(|e| {
                    log::warn!("Error in create_platform_window_surface: {:?}", e);
                    w::InitError::UnsupportedWindowHandle
                })
        }?;

        let wl_window = match has_handle.raw_window_handle() {
            #[cfg(not(target_os = "android"))]
            Rwh::Wayland(_) => Some(native_window_ptr),
            _ => None,
        };

        Ok(Surface {
            egl: inner.egl.clone(),
            raw,
            display: inner.display,
            context: inner.context,
            presentable: inner.supports_native_window,
            pbuffer: inner.pbuffer,
            wl_window,
            swapchain: None,
        })
    }

    unsafe fn destroy_surface(&self, surface: Surface) {
        let inner = self.inner.lock();
        inner
            .egl
            .destroy_surface(inner.display, surface.raw)
            .unwrap();
        if let Some(wl_window) = surface.wl_window {
            let wl_egl_window_destroy: libloading::Symbol<WlEglWindowDestroyFun> = self
                .wsi_library
                .as_ref()
                .expect("unsupported window")
                .get(b"wl_egl_window_destroy")
                .unwrap();
            wl_egl_window_destroy(wl_window)
        }
    }
}

#[derive(Debug)]
pub struct Surface {
    egl: Starc<egl::DynamicInstance<egl::EGL1_4>>,
    raw: egl::Surface,
    display: egl::Display,
    context: egl::Context,
    pbuffer: egl::Surface,
    presentable: bool,
    wl_window: Option<*mut raw::c_void>,
    pub(crate) swapchain: Option<Swapchain>,
}

unsafe impl Send for Surface {}
unsafe impl Sync for Surface {}

impl w::PresentationSurface<crate::Backend> for Surface {
    type SwapchainImage = native::SwapchainImage;

    unsafe fn configure_swapchain(
        &mut self,
        device: &crate::Device,
        config: w::SwapchainConfig,
    ) -> Result<(), w::SwapchainError> {
        self.unconfigure_swapchain(device);

        if let Some(window) = self.wl_window {
            let library = libloading::Library::new("libwayland-egl.so").unwrap();
            let wl_egl_window_resize: libloading::Symbol<WlEglWindowResizeFun> =
                library.get(b"wl_egl_window_resize").unwrap();
            wl_egl_window_resize(
                window,
                config.extent.width as i32,
                config.extent.height as i32,
                0,
                0,
            );
        }

        let desc = conv::describe_format(config.format).unwrap();

        let gl = &device.share.context;
        let renderbuffer = gl.create_renderbuffer().unwrap();
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(renderbuffer));
        gl.renderbuffer_storage(
            glow::RENDERBUFFER,
            desc.tex_internal,
            config.extent.width as _,
            config.extent.height as _,
        );
        let framebuffer = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_renderbuffer(
            glow::READ_FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::RENDERBUFFER,
            Some(renderbuffer),
        );
        gl.bind_renderbuffer(glow::RENDERBUFFER, None);
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);

        self.swapchain = Some(Swapchain {
            renderbuffer,
            framebuffer,
            extent: config.extent,
            format: desc.tex_internal,
            channel: config.format.base_format().1,
        });

        Ok(())
    }

    unsafe fn unconfigure_swapchain(&mut self, device: &crate::Device) {
        let gl = &device.share.context;
        if let Some(sc) = self.swapchain.take() {
            gl.delete_renderbuffer(sc.renderbuffer);
            gl.delete_framebuffer(sc.framebuffer);
        }
    }

    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
    ) -> Result<(Self::SwapchainImage, Option<w::Suboptimal>), w::AcquireError> {
        let sc = self.swapchain.as_ref().unwrap();
        let sc_image =
            native::SwapchainImage::new(sc.renderbuffer, sc.format, sc.extent, sc.channel);
        Ok((sc_image, None))
    }
}

impl w::Surface<crate::Backend> for Surface {
    fn supports_queue_family(&self, _: &crate::QueueFamily) -> bool {
        self.presentable
    }

    fn capabilities(&self, _physical_device: &PhysicalDevice) -> w::SurfaceCapabilities {
        w::SurfaceCapabilities {
            present_modes: w::PresentMode::FIFO,                  //TODO
            composite_alpha_modes: w::CompositeAlphaMode::OPAQUE, //TODO
            image_count: 2..=2,
            current_extent: None,
            extents: w::Extent2D {
                width: 4,
                height: 4,
            }..=w::Extent2D {
                width: 4096,
                height: 4096,
            },
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT,
        }
    }

    fn supported_formats(
        &self,
        _physical_device: &PhysicalDevice,
    ) -> Option<Vec<hal::format::Format>> {
        use hal::format::Format;
        Some(vec![Format::Rgba8Srgb, Format::Bgra8Srgb])
    }
}

impl Surface {
    pub(crate) unsafe fn present(
        &mut self,
        _image: native::SwapchainImage,
        gl: &GlContainer,
    ) -> Result<Option<w::Suboptimal>, w::PresentError> {
        let sc = self.swapchain.as_ref().unwrap();

        self.egl
            .make_current(
                self.display,
                Some(self.raw),
                Some(self.raw),
                Some(self.context),
            )
            .unwrap();
        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(sc.framebuffer));
        gl.blit_framebuffer(
            0,
            0,
            sc.extent.width as _,
            sc.extent.height as _,
            0,
            0,
            sc.extent.width as _,
            sc.extent.height as _,
            glow::COLOR_BUFFER_BIT,
            glow::NEAREST,
        );
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);

        self.egl.swap_buffers(self.display, self.raw).unwrap();

        self.egl
            .make_current(
                self.display,
                Some(self.pbuffer),
                Some(self.pbuffer),
                Some(self.context),
            )
            .unwrap();

        Ok(None)
    }
}
