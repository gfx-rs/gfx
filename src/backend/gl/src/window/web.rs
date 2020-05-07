use crate::{
    conv,
    device::Device,
    native,
    Backend as B,
    GlContainer,
    PhysicalDevice,
    QueueFamily,
    Starc,
};
use arrayvec::ArrayVec;
use glow::HasContext;
use hal::{adapter::Adapter, format as f, image, window};
use std::iter;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug)]
pub struct Swapchain {
    pub(crate) extent: window::Extent2D,
    pub(crate) fbos: ArrayVec<[native::RawFrameBuffer; 3]>,
}

impl window::Swapchain<B> for Swapchain {
    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
        _semaphore: Option<&native::Semaphore>,
        _fence: Option<&native::Fence>,
    ) -> Result<(window::SwapImageIndex, Option<window::Suboptimal>), window::AcquireError> {
        // TODO: sync
        Ok((0, None))
    }
}

#[derive(Clone, Debug)]
pub struct Surface {
    canvas: Starc<web_sys::HtmlCanvasElement>,
    pub(crate) swapchain: Option<Swapchain>,
    renderbuffer: Option<native::Renderbuffer>,
}

impl Surface {
    pub fn from_canvas(canvas: web_sys::HtmlCanvasElement) -> Self {
        Surface {
            canvas: Starc::new(canvas),
            swapchain: None,
            renderbuffer: None,
        }
    }

    pub fn from_raw_handle(has_handle: &impl raw_window_handle::HasRawWindowHandle) -> Self {
        if let raw_window_handle::RawWindowHandle::Web(handle) = has_handle.raw_window_handle() {
            let canvas = web_sys::window()
                .and_then(|win| win.document())
                .expect("Cannot get document")
                .query_selector(&format!("canvas[data-raw-handle=\"{}\"]", handle.id))
                .expect("Cannot query for canvas")
                .expect("Canvas is not found")
                .dyn_into()
                .expect("Failed to downcast to canvas type");
            Self::from_canvas(canvas)
        } else {
            unreachable!()
        }
    }

    fn swapchain_formats(&self) -> Vec<f::Format> {
        vec![f::Format::Rgba8Unorm, f::Format::Bgra8Unorm]
    }
}

impl window::Surface<B> for Surface {
    fn supports_queue_family(&self, _: &QueueFamily) -> bool {
        true
    }

    fn capabilities(&self, _physical_device: &PhysicalDevice) -> window::SurfaceCapabilities {
        let extent = hal::window::Extent2D {
            width: self.canvas.width(),
            height: self.canvas.height(),
        };

        window::SurfaceCapabilities {
            present_modes: window::PresentMode::FIFO, //TODO
            composite_alpha_modes: window::CompositeAlphaMode::OPAQUE, //TODO
            image_count: 1..=1,
            current_extent: Some(extent),
            extents: extent..=extent,
            max_image_layers: 1,
            usage: image::Usage::COLOR_ATTACHMENT | image::Usage::TRANSFER_SRC,
        }
    }

    fn supported_formats(&self, _physical_device: &PhysicalDevice) -> Option<Vec<f::Format>> {
        Some(self.swapchain_formats())
    }
}

impl window::PresentationSurface<B> for Surface {
    type SwapchainImage = native::ImageView;

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
            extent: config.extent,
            fbos: iter::once(fbo).collect(),
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
        let image = native::ImageView::Renderbuffer(self.renderbuffer.unwrap());
        Ok((image, None))
    }
}

impl hal::Instance<B> for Surface {
    fn create(_name: &str, _version: u32) -> Result<Self, hal::UnsupportedBackend> {
        unimplemented!()
    }

    fn enumerate_adapters(&self) -> Vec<Adapter<B>> {
        let adapter = PhysicalDevice::new_adapter((), GlContainer::from_canvas(&self.canvas)); // TODO: Move to `self` like native/window
        vec![adapter]
    }

    unsafe fn create_surface(
        &self,
        _: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, window::InitError> {
        unimplemented!()
    }

    unsafe fn destroy_surface(&self, _surface: Surface) {
        // TODO: Implement Surface cleanup
    }
}
