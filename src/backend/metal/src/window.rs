use {Adapter, Backend};
use {native, conversions};

use std::cell::RefCell;
use std::rc::Rc;

use core::{self, format, memory, image};
use core::{Backbuffer, SwapchainConfig};
use core::format::SurfaceType;
use core::format::ChannelType;
use core::CommandQueue;

use metal::*;
use objc::runtime::{Object, Class};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::{CFNumber, CFNumberRef};
use core_graphics::base::CGFloat;
use core_graphics::geometry::CGRect;
use io_surface::{self, IOSurface};

pub struct Surface(pub(crate) Rc<SurfaceInner>);

pub(crate) struct SurfaceInner {
    pub(crate) nsview: *mut Object,
    pub(crate) render_layer: RefCell<*mut Object>,
}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe { msg_send![self.nsview, release]; }
    }
}

pub struct Swapchain {
    surface: Rc<SurfaceInner>,
    pixel_width: u64,
    pixel_height: u64,

    io_surfaces: Vec<IOSurface>,
    frame_index: usize,
    present_index: usize,
}

const SWAP_CHAIN_IMAGE_COUNT: usize = 3;
const kCVPixelFormatType_32RGBA: u32 = (b'R' as u32) << 24 | (b'G' as u32) << 16 | (b'B' as u32) << 8 | b'A' as u32;

impl core::Surface<Backend> for Surface {
    fn get_kind(&self) -> image::Kind {
        unimplemented!()
    }

    fn surface_capabilities(&self, _: &Adapter) -> core::SurfaceCapabilities {
        unimplemented!()
    }

    fn supports_queue(&self, queue_family: &native::QueueFamily) -> bool {
        true // TODO: Not sure this is the case, don't know associativity of IOSurface
    }

    fn build_swapchain<C>(&mut self,
        config: SwapchainConfig,
        present_queue: &CommandQueue<Backend, C>,
    ) -> (Swapchain, Backbuffer<Backend>) {
        let (mtl_format, cv_format) = match config.color_format {
            format::Format(SurfaceType::R8_G8_B8_A8, ChannelType::Srgb) => (MTLPixelFormat::RGBA8Unorm_sRGB, kCVPixelFormatType_32RGBA),
            _ => panic!("unsupported backbuffer format"), // TODO: more formats
        };

        let render_layer_borrow = self.0.render_layer.borrow_mut();
        let render_layer = *render_layer_borrow;
        let nsview = self.0.nsview;

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
            let pixel_width = (view_points_size.size.width * scale_factor) as u64;
            let pixel_height = (view_points_size.size.height * scale_factor) as u64;
            let pixel_size = conversions::get_format_bytes_per_pixel(mtl_format) as u64;

            info!("allocating {} IOSurface backbuffers of size {}x{} with pixel format 0x{:x}", SWAP_CHAIN_IMAGE_COUNT, pixel_width, pixel_height, cv_format);
            // Create swap chain surfaces
            let io_surfaces: Vec<_> = (0..SWAP_CHAIN_IMAGE_COUNT).map(|_| {
                io_surface::new(&CFDictionary::from_CFType_pairs::<CFStringRef, CFNumberRef, CFString, CFNumber>(&[
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceWidth), CFNumber::from_i32(pixel_width as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceHeight), CFNumber::from_i32(pixel_height as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceBytesPerRow), CFNumber::from_i32((pixel_width * pixel_size) as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfaceBytesPerElement), CFNumber::from_i32(pixel_size as i32)),
                    (TCFType::wrap_under_get_rule(io_surface::kIOSurfacePixelFormat), CFNumber::from_i32(cv_format as i32)),
                ]))
            }).collect();

            let device = present_queue.as_raw().device();

            let backbuffer_descriptor = MTLTextureDescriptor::new();
            defer! { backbuffer_descriptor.release() };
            backbuffer_descriptor.set_pixel_format(mtl_format);
            backbuffer_descriptor.set_width(pixel_width as u64);
            backbuffer_descriptor.set_height(pixel_height as u64);
            backbuffer_descriptor.set_usage(MTLTextureUsageRenderTarget);

            let images = io_surfaces.iter().map(|surface| {
                let mapped_texture: MTLTexture = msg_send![device.0,
                    newTextureWithDescriptor: backbuffer_descriptor.0
                    iosurface: surface.obj
                    plane: 0
                ]; // Returns retained
                native::Image(mapped_texture)
            }).collect();

            let swapchain = Swapchain {
                surface: self.0.clone(),
                pixel_width,
                pixel_height,

                io_surfaces,
                frame_index: 0,
                present_index: 0,
            };

            (swapchain, Backbuffer::Images(images))
        }
    }
}

impl core::Swapchain<Backend> for Swapchain {
    fn acquire_frame(&mut self, sync: core::FrameSync<Backend>) -> core::Frame {
        unsafe {
            match sync {
                core::FrameSync::Semaphore(semaphore) => {
                    // FIXME: this is definitely wrong
                    native::dispatch_semaphore_signal(semaphore.0);
                },
                core::FrameSync::Fence(_fence) => unimplemented!(),
            }

            let frame = core::Frame::new(self.frame_index % self.io_surfaces.len());
            self.frame_index += 1;
            frame
        }
    }

    fn present<C>(
        &mut self,
        present_queue: &mut CommandQueue<Backend, C>,
        wait_semaphores: &[&native::Semaphore],
    ) {
        let buffer_index = self.present_index % self.io_surfaces.len();

        unsafe {
            let io_surface = &mut self.io_surfaces[buffer_index];
            let render_layer_borrow = self.surface.render_layer.borrow_mut();
            let render_layer = *render_layer_borrow;
            msg_send![render_layer, setContents: io_surface.obj];
        }

        self.present_index += 1;
    }
}

