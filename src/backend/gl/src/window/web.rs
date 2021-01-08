use crate::{
    conv, device::Device, native, Backend as B, GlContainer, PhysicalDevice, QueueFamily, Starc,
};
use glow::HasContext;
use hal::{adapter::Adapter, format as f, image, window};
use parking_lot::Mutex;
use std::sync::Arc;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug)]
pub struct Swapchain {
    pub(crate) extent: window::Extent2D,
    pub(crate) channel: f::ChannelType,
    pub(crate) raw_format: native::TextureFormat,
    pub(crate) framebuffer: native::RawFramebuffer,
}

#[derive(Debug)]
pub struct Instance {
    canvas: Mutex<Option<Starc<web_sys::HtmlCanvasElement>>>,
}

impl hal::Instance<B> for Instance {
    fn create(_name: &str, _version: u32) -> Result<Self, hal::UnsupportedBackend> {
        Ok(Instance {
            canvas: Mutex::new(None),
        })
    }

    fn enumerate_adapters(&self) -> Vec<Adapter<B>> {
        let canvas_guard = self.canvas.lock();
        let context = match *canvas_guard {
            Some(ref canvas) => {
                // TODO: Remove hardcoded width/height
                if canvas.get_attribute("width").is_none() {
                    canvas
                        .set_attribute("width", "640")
                        .expect("Cannot set width");
                }
                if canvas.get_attribute("height").is_none() {
                    canvas
                        .set_attribute("height", "480")
                        .expect("Cannot set height");
                }
                let context_options = js_sys::Object::new();
                js_sys::Reflect::set(
                    &context_options,
                    &"antialias".into(),
                    &wasm_bindgen::JsValue::FALSE,
                )
                .expect("Cannot create context options");
                let webgl2_context = canvas
                    .get_context_with_context_options("webgl2", &context_options)
                    .expect("Cannot create WebGL2 context")
                    .and_then(|context| context.dyn_into::<web_sys::WebGl2RenderingContext>().ok())
                    .expect("Cannot convert into WebGL2 context");
                glow::Context::from_webgl2_context(webgl2_context)
            }
            None => return Vec::new(),
        };

        let adapter = PhysicalDevice::new_adapter(context);
        vec![adapter]
    }

    unsafe fn create_surface(
        &self,
        has_handle: &impl raw_window_handle::HasRawWindowHandle,
    ) -> Result<Surface, window::InitError> {
        if let raw_window_handle::RawWindowHandle::Web(handle) = has_handle.raw_window_handle() {
            let canvas: Starc<web_sys::HtmlCanvasElement> = Starc::new(
                web_sys::window()
                    .and_then(|win| win.document())
                    .expect("Cannot get document")
                    .query_selector(&format!("canvas[data-raw-handle=\"{}\"]", handle.id))
                    .expect("Cannot query for canvas")
                    .expect("Canvas is not found")
                    .dyn_into()
                    .expect("Failed to downcast to canvas type"),
            );

            *self.canvas.lock() = Some(canvas.clone());

            Ok(Surface {
                canvas,
                swapchain: None,
                renderbuffer: None,
            })
        } else {
            unreachable!()
        }
    }

    unsafe fn destroy_surface(&self, surface: Surface) {
        let mut canvas_option_ref = self.canvas.lock();

        if let Some(canvas) = canvas_option_ref.as_ref() {
            if Arc::ptr_eq(&canvas.arc, &surface.canvas.arc) {
                *canvas_option_ref = None;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Surface {
    canvas: Starc<web_sys::HtmlCanvasElement>,
    pub(crate) swapchain: Option<Swapchain>,
    renderbuffer: Option<native::Renderbuffer>,
}

impl Surface {
    fn swapchain_formats(&self) -> Vec<f::Format> {
        vec![f::Format::Rgba8Unorm, f::Format::Bgra8Unorm]
    }

    pub(crate) unsafe fn present(
        &mut self,
        _image: native::SwapchainImage,
        gl: &GlContainer,
    ) -> Result<Option<window::Suboptimal>, window::PresentError> {
        let swapchain = self.swapchain.as_ref().unwrap();

        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(swapchain.framebuffer));
        gl.blit_framebuffer(
            0,
            0,
            swapchain.extent.width as _,
            swapchain.extent.height as _,
            0,
            0,
            swapchain.extent.width as _,
            swapchain.extent.height as _,
            glow::COLOR_BUFFER_BIT,
            glow::NEAREST,
        );
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);

        Ok(None)
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
    type SwapchainImage = native::SwapchainImage;

    unsafe fn configure_swapchain(
        &mut self,
        device: &Device,
        config: window::SwapchainConfig,
    ) -> Result<(), window::SwapchainError> {
        let gl = &device.share.context;

        if let Some(swapchain) = self.swapchain.take() {
            // delete all frame buffers already allocated
            gl.delete_framebuffer(swapchain.framebuffer);
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

        let framebuffer = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_renderbuffer(
            glow::READ_FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::RENDERBUFFER,
            self.renderbuffer,
        );
        self.swapchain = Some(Swapchain {
            extent: config.extent,
            channel: config.format.base_format().1,
            raw_format: desc.tex_external,
            framebuffer,
        });
        Ok(())
    }

    unsafe fn unconfigure_swapchain(&mut self, device: &Device) {
        let gl = &device.share.context;
        if let Some(swapchain) = self.swapchain.take() {
            gl.delete_framebuffer(swapchain.framebuffer);
        }
        if let Some(renderbuffer) = self.renderbuffer.take() {
            gl.delete_renderbuffer(renderbuffer);
        }
    }

    unsafe fn acquire_image(
        &mut self,
        _timeout_ns: u64,
    ) -> Result<(Self::SwapchainImage, Option<window::Suboptimal>), window::AcquireError> {
        let sc = self.swapchain.as_ref().unwrap();
        let swapchain_image = native::SwapchainImage::new(
            self.renderbuffer.unwrap(),
            sc.raw_format,
            sc.extent,
            sc.channel,
        );
        Ok((swapchain_image, None))
    }
}
