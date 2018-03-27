use {Backend, QueueFamily};
use {native, conversions};
use device::{Device, PhysicalDevice};

use std::sync::{Arc, Mutex};

use hal::{self, format, image};
use hal::{Backbuffer, SwapchainConfig};
use hal::window::Extent2D;

use metal::{self, MTLPixelFormat, MTLTextureUsage};
use objc::runtime::{Object};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::{CFNumber, CFNumberRef};
use core_graphics::base::CGFloat;
use core_graphics::geometry::CGRect;
use cocoa::foundation::{NSRect};
use io_surface::{self, IOSurface};


pub struct Surface(pub(crate) Arc<SurfaceInner>);

pub(crate) struct SurfaceInner {
    pub(crate) nsview: *mut Object,
    pub(crate) render_layer: Mutex<*mut Object>,
}

unsafe impl Send for SurfaceInner {}
unsafe impl Sync for SurfaceInner {}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe { msg_send![self.nsview, release]; }
    }
}

pub struct Swapchain {
     surface: Arc<SurfaceInner>,
    _size_pixels: (u64, u64),
    io_surfaces: Vec<IOSurface>,
    frame_index: usize,
    present_index: usize,
}

unsafe impl Send for Swapchain {}
unsafe impl Sync for Swapchain {}

impl Swapchain {
    pub(crate) fn present(&mut self) -> (&SurfaceInner, &mut IOSurface) {
        let id = self.present_index;
        self.present_index = (id + 1) % self.io_surfaces.len();
        (&*self.surface, &mut self.io_surfaces[id])
    }
}

#[allow(bad_style)]
const kCVPixelFormatType_32RGBA: u32 = (b'R' as u32) << 24 | (b'G' as u32) << 16 | (b'B' as u32) << 8 | b'A' as u32;

impl hal::Surface<Backend> for Surface {
    fn kind(&self) -> image::Kind {
        let (width, height) = self.pixel_dimensions();

        image::Kind::D2(width, height, 1, 1)
    }

    fn capabilities_and_formats(
        &self, _: &PhysicalDevice,
    ) -> (hal::SurfaceCapabilities, Option<Vec<format::Format>>) {
        let caps = hal::SurfaceCapabilities {
            image_count: 1..8,
            current_extent: None,
            extents: Extent2D { width: 4, height: 4} .. Extent2D { width: 4096, height: 4096 },
            max_image_layers: 1,
        };
        let formats = Some(vec![format::Format::Rgba8Srgb]);
        (caps, formats)
    }

    fn supports_queue_family(&self, _queue_family: &QueueFamily) -> bool {
        true // TODO: Not sure this is the case, don't know associativity of IOSurface
    }
}

impl Surface {
    fn pixel_dimensions(&self) -> (image::Size, image::Size) {
        unsafe {
            // NSView bounds are measured in DIPs
            let bounds: NSRect = msg_send![self.0.nsview, bounds];
            (bounds.size.width as _, bounds.size.height as _)
        }
    }
}

impl Device {
    pub fn build_swapchain(
        &self,
        surface: &mut Surface,
        config: SwapchainConfig,
    ) -> (Swapchain, Backbuffer<Backend>) {
        let format_desc = config.color_format.base_format().0.desc();
        let (mtl_format, cv_format) = match config.color_format {
            format::Format::Rgba8Srgb => (MTLPixelFormat::RGBA8Unorm_sRGB, kCVPixelFormatType_32RGBA),
            _ => panic!("unsupported backbuffer format"), // TODO: more formats
        };

        let render_layer_borrow = surface.0.render_layer.lock().unwrap();
        let render_layer = *render_layer_borrow;
        let nsview = surface.0.nsview;

        unsafe {
            // Update render layer size
            let view_points_size: CGRect = msg_send![nsview, bounds];
            msg_send![render_layer, setBounds: view_points_size];
            let view_window: *mut Object = msg_send![nsview, window];
            if view_window.is_null() {
                panic!("surface is not attached to a window");
            }
            let scale_factor: CGFloat = msg_send![view_window, backingScaleFactor];
            msg_send![render_layer, setContentsScale: scale_factor];
            let pixel_width = view_points_size.size.width as u64;
            let pixel_height = view_points_size.size.height as u64;
            let pixel_size = conversions::get_format_bytes_per_pixel(mtl_format) as i32;

            info!("allocating {} IOSurface backbuffers of size {}x{} with pixel format 0x{:x}", config.image_count, pixel_width, pixel_height, cv_format);
            // Create swap chain surfaces
            let io_surfaces: Vec<_> = (0..config.image_count).map(|_| {
                io_surface::new(&CFDictionary::from_CFType_pairs::<CFStringRef, CFNumberRef, CFString, CFNumber>(&[
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceWidth), CFNumber::from(pixel_width as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceHeight), CFNumber::from(pixel_height as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceBytesPerRow), CFNumber::from(pixel_width as i32 * pixel_size)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceBytesPerElement), CFNumber::from(pixel_size)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfacePixelFormat), CFNumber::from(cv_format as i32)),
                ]))
            }).collect();

            let backbuffer_descriptor = metal::TextureDescriptor::new();
            backbuffer_descriptor.set_pixel_format(mtl_format);
            backbuffer_descriptor.set_width(pixel_width);
            backbuffer_descriptor.set_height(pixel_height);
            backbuffer_descriptor.set_usage(MTLTextureUsage::MTLTextureUsageRenderTarget);

            let images = io_surfaces.iter().map(|surface| {
                let mapped_texture: metal::Texture = msg_send![self.device.as_ref(),
                    newTextureWithDescriptor: &*backbuffer_descriptor
                    iosurface: surface.obj
                    plane: 0
                ];
                native::Image {
                    raw: mapped_texture,
                    format_desc,
                }
            }).collect();

            let swapchain = Swapchain {
                surface: surface.0.clone(),
                _size_pixels: (pixel_width, pixel_height),
                io_surfaces,
                frame_index: 0,
                present_index: 0,
            };

            (swapchain, Backbuffer::Images(images))
        }
    }
}

impl hal::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, sync: hal::FrameSync<Backend>) -> hal::Frame {
        unsafe {
            match sync {
                hal::FrameSync::Semaphore(semaphore) => {
                    // FIXME: this is definitely wrong
                    native::dispatch_semaphore_signal(semaphore.0);
                },
                hal::FrameSync::Fence(_fence) => unimplemented!(),
            }

            let frame = hal::Frame::new(self.frame_index % self.io_surfaces.len());
            self.frame_index += 1;
            frame
        }
    }
}

