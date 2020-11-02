//! [Surfman](https://github.com/pcwalton/surfman)-based OpenGL backend for GFX-hal

use crate::{conv, native, Backend as B, Device, GlContainer, PhysicalDevice, QueueFamily, Starc};
use hal::{adapter::Adapter, format as f, image, window};

use arrayvec::ArrayVec;
use glow::HasContext;
use parking_lot::RwLock;
use surfman as sm;

use std::cell::RefCell;
use std::fmt;
use std::iter;

#[derive(Debug)]
pub struct Swapchain {
    // Underlying window, required for presentation
    pub(crate) context: Starc<RwLock<sm::Context>>,
    // Extent because the window lies
    pub(crate) extent: window::Extent2D,
    /// Channel type for the swapchain.
    pub(crate) channel: f::ChannelType,
    ///
    pub(crate) raw_format: native::TextureFormat,
    ///
    pub(crate) fbos: ArrayVec<[native::RawFrameBuffer; 3]>,
    /// Renderbuffer
    pub(crate) renderbuffer: native::Renderbuffer
}

thread_local! {
    /// The thread-local surfman connection
    static SM_CONN: RefCell<sm::Connection> =
        RefCell::new(sm::Connection::new().expect("TODO"));
}

pub struct Instance {
    // The root context with which all other context share resources
    root_context: Starc<RwLock<sm::Context>>,
    device: Starc<RwLock<sm::Device>>,
}

impl fmt::Debug for Instance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Instance").field(&["Adapter..."; 3]).finish()
    }
}

impl Instance {
    fn get_default_context_attributes() -> sm::ContextAttributes {
        sm::ContextAttributes {
            version: sm::GLVersion::new(3, 3), // TODO: Figure out how to determine GL version
            // TODO: Skipping COMPATIBILITY_PROFILE for now, because it panics with a TODO, but
            // that is probably something we want to provide later.
            flags: sm::ContextAttributeFlags::ALPHA,
        }
    }

    pub unsafe fn create_surface_from_rwh(
        &self,
        raw_handle: raw_window_handle::RawWindowHandle,
    ) -> Surface {
        // Get write access to the device
        let mut device = self.device.write();

        // Get context attributes
        let context_attributes = Self::get_default_context_attributes();

        // Share the context with the root context
        let root_ctx = &*self.root_context.read();

        // Create context descriptor
        let context_descriptor = device
            .create_context_descriptor(&context_attributes)
            .expect("TODO");

        // Create context
        let mut context = device
            .create_context(&context_descriptor, Some(root_ctx))
            .expect("TODO");

        // Create the surface with the context
        let surface = device
            .create_surface(
                &context,
                surfman::SurfaceAccess::GPUOnly,
                surfman::SurfaceType::Widget {
                    // Create a native widget for the raw window handle
                    native_widget: SM_CONN.with(|c| {
                        c.borrow()
                            .create_native_widget_from_rwh(raw_handle)
                            .expect("TODO")
                    }),
                },
            )
            .expect("TODO");

        // Bind surface to context
        device
            .bind_surface_to_context(&mut context, surface)
            .expect("TODO");

        device.make_context_current(&context).expect("TODO");

        // Create a surface with the given context
        Surface {
            renderbuffer: None,
            swapchain: None,
            surface_context: Starc::new(RwLock::new(context)),
        }
    }
}

impl hal::Instance<B> for Instance {
    fn create(_: &str, _: u32) -> Result<Self, hal::UnsupportedBackend> {
        // Create adapter, device, and root context
        let (device, root_context) = SM_CONN.with(|c| {
            let conn = c.borrow();
            // TODO: Assume hardware adapter. We should also be able to create
            // low-power and software adapters.
            let adapter = conn.create_hardware_adapter().expect("TODO");
            let mut device = conn.create_device(&adapter).expect("TODO");
            let context_descriptor = device
                .create_context_descriptor(&Self::get_default_context_attributes())
                .expect("TODO");
            let context = device
                .create_context(&context_descriptor, None)
                .expect("TODO");

            (
                Starc::new(RwLock::new(device)),
                Starc::new(RwLock::new(context)),
            )
        });

        Ok(Instance {
            device,
            root_context,
        })
    }

    fn enumerate_adapters(&self) -> Vec<Adapter<B>> {
        // Make context current
        self.device
            .read()
            .make_context_current(&self.root_context.read())
            .expect("TODO");

        // Create gl container
        let gl = GlContainer::from_fn_proc(
            |symbol_name| {
                self.device
                    .write()
                    .get_proc_address(&self.root_context.read(), symbol_name)
                    as *const _
            },
            self.device.clone(),
            self.root_context.clone(),
        );

        // Create physical device
        vec![PhysicalDevice::new_adapter((), gl)]
    }

    unsafe fn create_surface(
        &self,
        has_handle: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, window::InitError> {
        Ok(self.create_surface_from_rwh(has_handle.raw_window_handle()))
    }

    unsafe fn destroy_surface(&self, surface: Surface) {
        // Unbind and get the underlying surface from the context
        let raw_surface = self
            .device
            .read()
            .unbind_surface_from_context(&mut surface.surface_context.write())
            .expect("TODO");

        if let Some(mut raw_surface) = raw_surface {
            // Destroy the underlying surface
            self.device
                .read()
                .destroy_surface(&mut surface.surface_context.write(), &mut raw_surface)
                .expect("TODO");
        }

        // Destroy the backing context
        self.device
            .read()
            .destroy_context(&mut surface.surface_context.write())
            .expect("TODO");
    }
}

#[derive(Debug)]
pub struct Surface {
    pub(crate) swapchain: Option<Swapchain>,
    pub(crate) surface_context: Starc<RwLock<sm::Context>>,
    renderbuffer: Option<native::Renderbuffer>,
}

impl Surface {
    pub fn context(&self) -> Starc<RwLock<sm::Context>> {
        self.surface_context.clone()
    }

    fn swapchain_formats(&self) -> Vec<f::Format> {
        // TODO: Make sure this is correct. I ( @zicklag ) believe it is. Reference:
        // https://github.com/pcwalton/surfman/blob/master/surfman/src/context.rs#L34-L37
        vec![f::Format::Rgba8Srgb, f::Format::Bgra8Srgb]
    }
}

impl window::PresentationSurface<B> for Surface {
    type SwapchainImage = native::SwapchainImage;

    unsafe fn configure_swapchain(
        &mut self,
        device: &Device,
        config: window::SwapchainConfig,
    ) -> Result<(), window::CreationError> {
        let gl = &device.share.context;

        if let Some(old) = self.swapchain.take() {
            for fbo in old.fbos {
                gl.delete_framebuffer(fbo);
            }
        }

        if self.renderbuffer.is_none() {
            self.renderbuffer = Some(gl.create_renderbuffer().unwrap());
        }

        let desc = conv::describe_format(config.format).unwrap();
        gl.bind_renderbuffer(glow::RENDERBUFFER, self.renderbuffer);
        gl.renderbuffer_storage(
            glow::RENDERBUFFER,
            desc.tex_internal,
            config.extent.width as i32,
            config.extent.height as i32,
        );

        let fbo = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fbo));
        gl.framebuffer_renderbuffer(
            glow::READ_FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::RENDERBUFFER,
            self.renderbuffer,
        );
        self.swapchain = Some(Swapchain {
            context: self.surface_context.clone(),
            extent: config.extent,
            channel: config.format.base_format().1,
            raw_format: desc.tex_external,
            fbos: iter::once(fbo).collect(),
            renderbuffer: self.renderbuffer.unwrap(),
        });

        Ok(())
    }

    unsafe fn unconfigure_swapchain(&mut self, device: &Device) {
        let gl = &device.share.context;
        if let Some(old) = self.swapchain.take() {
            for fbo in old.fbos {
                gl.delete_framebuffer(fbo);
            }
        }
        if let Some(rbo) = self.renderbuffer.take() {
            gl.delete_renderbuffer(rbo);
        }
    }

    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
    ) -> Result<(Self::SwapchainImage, Option<window::Suboptimal>), window::AcquireError> {
        let sc = self.swapchain.as_ref().unwrap();
        let swapchain_image =
            native::SwapchainImage::new(self.renderbuffer.unwrap(), sc.raw_format, sc.channel);
        Ok((swapchain_image, None))
    }
}

impl window::Surface<B> for Surface {
    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        true
    }

    fn capabilities(&self, _physical_device: &PhysicalDevice) -> window::SurfaceCapabilities {
        window::SurfaceCapabilities {
            present_modes: window::PresentMode::FIFO, //TODO
            composite_alpha_modes: window::CompositeAlphaMode::OPAQUE, //TODO
            // TODO: Figure out how to get pixel format from surfman
            // image_count: if self.context.get_pixel_format().double_buffer {
            //     2..=2
            // } else {
            //     1..=1
            // },
            image_count: 2..=2,
            current_extent: None,
            extents: window::Extent2D {
                width: 4,
                height: 4,
            }..=window::Extent2D {
                width: 4096,
                height: 4096,
            },
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT | image::Usage::TRANSFER_SRC,
        }
    }

    fn supported_formats(&self, _physical_device: &PhysicalDevice) -> Option<Vec<f::Format>> {
        Some(self.swapchain_formats())
    }
}
