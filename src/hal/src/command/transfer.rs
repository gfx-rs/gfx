use std::borrow::Borrow;
use std::ops::Range;
use Backend;
use {format, image};
use device::Extent;
use memory::Barrier;
use pso::PipelineStage;
use queue::capability::{Supports, Transfer};
use super::{CommandBuffer, RawCommandBuffer, Shot, Level};


///
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Offset {
    ///
    pub x: i32,
    ///
    pub y: i32,
    ///
    pub z: i32,
}

/// Region of two buffers for copying.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BufferCopy {
    /// Buffer region source offset.
    pub src: u64,
    /// Buffer region destination offset.
    pub dst: u64,
    /// Region size.
    pub size: u64,
}

///
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageCopy {
    ///
    pub aspect_mask: format::AspectFlags,
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BufferImageCopy {
    /// Buffer ofset in bytes.
    pub buffer_offset: u64,
    /// Width of a buffer 'row' in texels.
    pub buffer_width: u32,
    /// Height of a buffer 'image slice' in texels.
    pub buffer_height: u32,
    ///
    pub image_layers: image::SubresourceLayers,
    ///
    pub image_offset: Offset,
    ///
    pub image_extent: Extent,
}


impl<'a, B: Backend, C: Supports<Transfer>, S: Shot, L: Level> CommandBuffer<'a, B, C, S, L> {
    ///
    pub fn pipeline_barrier<'i, T>(
        &mut self,
        stages: Range<PipelineStage>,
        barriers: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<Barrier<'i, B>>,
    {
        self.raw.pipeline_barrier(stages, barriers)
    }


    ///
    pub fn fill_buffer(
        &mut self,
        buffer: &B::Buffer,
        range: Range<u64>,
        data: u32,
    ) {
        self.raw.fill_buffer(buffer, range, data)
    }

    ///
    pub fn copy_buffer<T>(
        &mut self,
        src: &B::Buffer,
        dst: &B::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferCopy>,
    {
        self.raw.copy_buffer(src, dst, regions)
    }

    ///
    pub fn update_buffer(
        &mut self,
        buffer: &B::Buffer,
        offset: u64,
        data: &[u8],
    ) {
        self.raw.update_buffer(buffer, offset, data)
    }

    ///
    pub fn copy_image<T>(
        &mut self,
        src: &B::Image,
        src_layout: image::ImageLayout,
        dst: &B::Image,
        dst_layout: image::ImageLayout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageCopy>,
    {
        self.raw.copy_image(src, src_layout, dst, dst_layout, regions)
    }

    ///
    pub fn copy_buffer_to_image<T>(
        &mut self,
        src: &B::Buffer,
        dst: &B::Image,
        dst_layout: image::ImageLayout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferImageCopy>,
    {
        self.raw.copy_buffer_to_image(src, dst, dst_layout, regions)
    }

    ///
    pub fn copy_image_to_buffer<T>(
        &mut self,
        src: &B::Image,
        src_layout: image::ImageLayout,
        dst: &B::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferImageCopy>,
    {
        self.raw.copy_image_to_buffer(src, src_layout, dst, regions)
    }
}
