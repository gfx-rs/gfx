//! `CommandBuffer` methods for transfer operations.
use std::borrow::Borrow;
use std::ops::{Range, RangeBounds};

use super::{CommandBuffer, Level, RawCommandBuffer, Shot};
use crate::memory::{Barrier, Dependencies};
use crate::pso::PipelineStage;
use crate::queue::capability::{Supports, Transfer};
use crate::range::RangeArg;
use crate::Backend;
use crate::{buffer, image};


/// Specifies a source region and a destination
/// region in a buffer for copying.  All values
/// are in units of bytes.
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

/// Bundles together all the parameters needed to copy data from one `Image`
/// to another.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageCopy {
    /// The image subresource to copy from.
    pub src_subresource: image::SubresourceLayers,
    /// The source offset.
    pub src_offset: image::Offset,
    /// The image subresource to copy to.
    pub dst_subresource: image::SubresourceLayers,
    /// The destination offset.
    pub dst_offset: image::Offset,
    /// The extent of the region to copy.
    pub extent: image::Extent,
}

/// Bundles together all the parameters needed to copy a buffer
/// to an image or vice-versa.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BufferImageCopy {
    /// Buffer offset in bytes.
    pub buffer_offset: buffer::Offset,
    /// Width of a buffer 'row' in texels.
    pub buffer_width: u32,
    /// Height of a buffer 'image slice' in texels.
    pub buffer_height: u32,
    /// The image subresource.
    pub image_layers: image::SubresourceLayers,
    /// The offset of the portion of the image to copy.
    pub image_offset: image::Offset,
    /// Size of the portion of the image to copy.
    pub image_extent: image::Extent,
}

impl<B: Backend, C: Supports<Transfer>, S: Shot, L: Level> CommandBuffer<B, C, S, L> {
    /// Identical to the `RawCommandBuffer` method of the same name.
    pub unsafe fn pipeline_barrier<'i, R, T>(
        &mut self,
        stages: Range<PipelineStage>,
        dependencies: Dependencies,
        barriers: T,
    ) where
        R: RangeBounds<buffer::Offset>,
        T: IntoIterator,
        T::Item: Borrow<Barrier<'i, B, R>>,
    {
        self.raw.pipeline_barrier(stages, dependencies, barriers)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub unsafe fn fill_buffer<R>(&mut self, buffer: &B::Buffer, range: R, data: u32)
    where
        R: RangeArg<buffer::Offset>,
    {
        self.raw.fill_buffer(buffer, range, data)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub unsafe fn copy_buffer<T>(&mut self, src: &B::Buffer, dst: &B::Buffer, regions: T)
    where
        T: IntoIterator,
        T::Item: Borrow<BufferCopy>,
    {
        self.raw.copy_buffer(src, dst, regions)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub unsafe fn update_buffer(
        &mut self,
        buffer: &B::Buffer,
        offset: buffer::Offset,
        data: &[u8],
    ) {
        self.raw.update_buffer(buffer, offset, data)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub unsafe fn copy_image<T>(
        &mut self,
        src: &B::Image,
        src_layout: image::Layout,
        dst: &B::Image,
        dst_layout: image::Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageCopy>,
    {
        self.raw
            .copy_image(src, src_layout, dst, dst_layout, regions)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub unsafe fn copy_buffer_to_image<T>(
        &mut self,
        src: &B::Buffer,
        dst: &B::Image,
        dst_layout: image::Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferImageCopy>,
    {
        self.raw.copy_buffer_to_image(src, dst, dst_layout, regions)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub unsafe fn copy_image_to_buffer<T>(
        &mut self,
        src: &B::Image,
        src_layout: image::Layout,
        dst: &B::Buffer,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<BufferImageCopy>,
    {
        self.raw.copy_image_to_buffer(src, src_layout, dst, regions)
    }
}
