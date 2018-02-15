use std::borrow::Borrow;
use std::ops::Range;
use Backend;
use {format, image};
use device::Extent;
use memory::Barrier;
use pso::PipelineStage;
use queue::capability::{Supports, Transfer};
use super::{CommandBuffer, RawCommandBuffer, Shot, Level};


/// An offset into an `Image`
/// DOC TODO: Are the y and z values undefined if the Image is only 1D or 2D,
/// or must they be set to a null value?
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Offset {
    /// X offset, in texels.
    pub x: i32,
    /// Y offset, in texels.
    pub y: i32,
    /// Z offset, in texels.
    pub z: i32,
}

/// Specifies a source region and a destination
/// region in a buffer for copying.  All values
/// are in bytes.
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

/// DOC TODO
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageResolve {
    /// DOC TODO
    pub src_subresource: image::Subresource,
    /// DOC TODO
    pub dst_subresource: image::Subresource,
    /// DOC TODO
    pub num_layers: image::Layer,
}

/// Bundles together all the data needed to copy data from one `Image`
/// to another.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageCopy {
    /// The aspect mask of what to copy: color, depth and/or stencil information.
    pub aspect_mask: format::AspectFlags,
    /// The image subresource to copy from.
    pub src_subresource: image::Subresource,
    /// The source offset.
    pub src_offset: Offset,
    /// The image subresource to copy from.
    pub dst_subresource: image::Subresource,
    /// The destination offset.
    pub dst_offset: Offset,
    /// DOC TODO
    pub extent: Extent,
    /// The number of layers to copy.
    pub num_layers: image::Layer,
}

/// DOC TODO
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BufferImageCopy {
    /// Buffer ofset in bytes.
    pub buffer_offset: u64,
    /// Width of a buffer 'row' in texels.
    pub buffer_width: u32,
    /// Height of a buffer 'image slice' in texels.
    pub buffer_height: u32,
    /// The number of layers to copy.
    pub image_layers: image::SubresourceLayers,
    /// The offset of the image.
    pub image_offset: Offset,
    /// DOC TODO
    pub image_extent: Extent,
}


impl<'a, B: Backend, C: Supports<Transfer>, S: Shot, L: Level> CommandBuffer<'a, B, C, S, L> {
    /// DOC TODO
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


    /// DOC TODO
    pub fn fill_buffer(
        &mut self,
        buffer: &B::Buffer,
        range: Range<u64>,
        data: u32,
    ) {
        self.raw.fill_buffer(buffer, range, data)
    }

    /// DOC TODO
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

    /// DOC TODO
    pub fn update_buffer(
        &mut self,
        buffer: &B::Buffer,
        offset: u64,
        data: &[u8],
    ) {
        self.raw.update_buffer(buffer, offset, data)
    }

    /// DOC TODO
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

    /// DOC TODO
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

    /// DOC TODO
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
