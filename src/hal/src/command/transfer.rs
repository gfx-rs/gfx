use std::borrow::Borrow;
use std::ops::Range;

use Backend;
use {buffer, format, image};
use device::Extent;
use memory::{Barrier, Dependencies};
use pso::PipelineStage;
use queue::capability::{Supports, Transfer};
use super::{CommandBuffer, RawCommandBuffer, Shot, Level};


/// Region of two buffers for copying.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BufferCopy {
    /// Buffer region source offset.
    pub src: buffer::Offset,
    /// Buffer region destination offset.
    pub dst: buffer::Offset,
    /// Region size.
    pub size: buffer::Offset,
}

///
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageCopy {
    ///
    pub aspects: format::Aspects,
    ///
    pub src_subresource: image::Subresource,
    ///
    pub src_offset: image::Offset,
    ///
    pub dst_subresource: image::Subresource,
    ///
    pub dst_offset: image::Offset,
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
    pub buffer_offset: buffer::Offset,
    /// Width of a buffer 'row' in texels.
    pub buffer_width: u32,
    /// Height of a buffer 'image slice' in texels.
    pub buffer_height: u32,
    ///
    pub image_layers: image::SubresourceLayers,
    ///
    pub image_offset: image::Offset,
    ///
    pub image_extent: Extent,
}


impl<'a, B: Backend, C: Supports<Transfer>, S: Shot, L: Level> CommandBuffer<'a, B, C, S, L> {
    ///
    pub fn pipeline_barrier<'i, T>(
        &mut self,
        stages: Range<PipelineStage>,
        dependencies: Dependencies,
        barriers: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<Barrier<'i, B>>,
    {
        self.raw.pipeline_barrier(stages, dependencies, barriers)
    }


    ///
    pub fn fill_buffer(
        &mut self,
        buffer: &B::Buffer,
        range: Range<buffer::Offset>,
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
        offset: buffer::Offset,
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
