
use Backend;
use image;
use memory::Barrier;
use queue::capability::{Supports, Transfer};
use super::{CommandBuffer, RawCommandBuffer};


///
#[derive(Clone, Copy, Debug)]
pub struct Offset {
    ///
    pub x: i32,
    ///
    pub y: i32,
    ///
    pub z: i32,
}

///
#[derive(Clone, Copy, Debug)]
pub struct Extent {
    ///
    pub width: u32,
    ///
    pub height: u32,
    ///
    pub depth: u32,
}

/// Region of two buffers for copying.
#[derive(Clone, Copy, Debug)]
pub struct BufferCopy {
    /// Buffer region source offset.
    pub src: u64,
    /// Buffer region destionation offset.
    pub dst: u64,
    /// Region size.
    pub size: u64,
}

///
#[derive(Clone, Debug)]
pub struct ImageResolve {
    ///
    pub src_subresource: image::Subresource,
    ///
    pub dst_subresource: image::Subresource,
    ///
    pub num_layers: image::Layer,
}

///
#[derive(Clone, Debug)]
pub struct ImageCopy {
    ///
    pub aspect_mask: image::AspectFlags,
    ///
    pub src_subresource: image::Subresource,
    ///
    pub src_offset: Offset,
    ///
    pub dst_subresource: image::Subresource,
    ///
    pub dst_offset: Offset,
    ///
    pub extent: Extent,
    ///
    pub num_layers: image::Layer,
}

///
#[derive(Clone, Debug)]
pub struct BufferImageCopy {
    ///
    pub buffer_offset: u64,
    ///
    pub buffer_row_pitch: u32,
    ///
    pub buffer_slice_pitch: u32,
    ///
    pub image_aspect: image::AspectFlags,
    ///
    pub image_subresource: image::SubresourceLayers,
    ///
    pub image_offset: Offset,
    ///
    pub image_extent: Extent,
}


impl<'a, B: Backend, C: Supports<Transfer>> CommandBuffer<'a, B, C> {
    ///
    pub fn pipeline_barrier(&mut self, barriers: &[Barrier<B>]) {
        self.raw.pipeline_barrier(barriers)
    }

    ///
    pub fn copy_buffer(&mut self, src: &B::Buffer, dst: &B::Buffer, regions: &[BufferCopy]) {
        self.raw.copy_buffer(src, dst, regions)
    }

    /*
    ///
    pub fn update_buffer(&mut self, buffer: &B::Buffer, data: &[u8], offset: usize) {
        self.raw.update_buffer(buffer, data, offset)
    }
    */

    ///
    pub fn copy_image(
        &mut self,
        src: &B::Image,
        src_layout: image::ImageLayout,
        dst: &B::Image,
        dst_layout: image::ImageLayout,
        regions: &[ImageCopy],
    ) {
        self.raw.copy_image(src, src_layout, dst, dst_layout, regions)
    }

    ///
    pub fn copy_buffer_to_image(
        &mut self,
        src: &B::Buffer,
        dst: &B::Image,
        layout: image::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        self.raw.copy_buffer_to_image(src, dst, layout, regions)
    }

    ///
    pub fn copy_image_to_buffer(
        &mut self,
        src: &B::Image,
        dst: &B::Buffer,
        layout: image::ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        self.raw.copy_image_to_buffer(src, dst, layout, regions)
    }
}
