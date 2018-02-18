use std::borrow::Borrow;
use std::ops::Range;

use Backend;
use {image, pso};
use buffer::IndexBufferView;
use device::Extent;
use query::{Query, QueryControl, QueryId};
use queue::capability::{Graphics, GraphicsOrCompute, Supports};
use super::{
    CommandBuffer, RawCommandBuffer, RenderPassInlineEncoder,
    RenderPassSecondaryEncoder, Shot, Level, Primary,
    ClearColorRaw, Offset,
};


#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Viewport {
    pub rect: Rect,
    pub depth: Range<f32>,
}

/// A single RGBA float color.
pub type ColorValue = [f32; 4];
/// A single depth value from a depth buffer.
pub type DepthValue = f32;
/// A single value from a stencil buffer.
pub type StencilValue = u32;

/// A universal clear color supporting integer formats
/// as well as the standard floating-point.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ClearColor {
    /// Standard floating-point `vec4` color
    Float(ColorValue),
    /// Integer vector to clear `ivec4` targets.
    Int([i32; 4]),
    /// Unsigned int vector to clear `uvec4` targets.
    Uint([u32; 4]),
}

macro_rules! impl_clear {
    { $( $ty:ty = $sub:ident[$a:expr, $b:expr, $c:expr, $d:expr], )* } => {
        $(
            impl From<$ty> for ClearColor {
                fn from(v: $ty) -> ClearColor {
                    ClearColor::$sub([v[$a], v[$b], v[$c], v[$d]])
                }
            }
        )*
    }
}

impl_clear! {
    [f32; 4] = Float[0, 1, 2, 3],
    [f32; 3] = Float[0, 1, 2, 0],
    [f32; 2] = Float[0, 1, 0, 0],
    [i32; 4] = Int  [0, 1, 2, 3],
    [i32; 3] = Int  [0, 1, 2, 0],
    [i32; 2] = Int  [0, 1, 0, 0],
    [u32; 4] = Uint [0, 1, 2, 3],
    [u32; 3] = Uint [0, 1, 2, 0],
    [u32; 2] = Uint [0, 1, 0, 0],
}

impl From<f32> for ClearColor {
    fn from(v: f32) -> ClearColor {
        ClearColor::Float([v, 0.0, 0.0, 0.0])
    }
}
impl From<i32> for ClearColor {
    fn from(v: i32) -> ClearColor {
        ClearColor::Int([v, 0, 0, 0])
    }
}
impl From<u32> for ClearColor {
    fn from(v: u32) -> ClearColor {
        ClearColor::Uint([v, 0, 0, 0])
    }
}

impl From<ClearColor> for ClearColorRaw {
    fn from(cv: ClearColor) -> Self {
        match cv {
            ClearColor::Float(cv) => ClearColorRaw { float32: cv },
            ClearColor::Int(cv) => ClearColorRaw { int32: cv },
            ClearColor::Uint(cv) => ClearColorRaw { uint32: cv },
        }
    }
}

/// Depth-stencil target clear values.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ClearDepthStencil(pub DepthValue, pub StencilValue);

/// General clear values for attachments (color or depth-stencil).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ClearValue {
    ///
    Color(ClearColor),
    ///
    DepthStencil(ClearDepthStencil),
}

/// Attachment clear description for the current subpass.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AttachmentClear {
    /// Clear color attachment.
    ///
    /// First tuple element denotes the index of the color attachment.
    Color(usize, ClearColor),
    /// Clear depth component of the attachment.
    Depth(DepthValue),
    /// Clear stencil component of the attachment.
    Stencil(StencilValue),
    /// Clear depth-stencil component of the attachment.
    DepthStencil(ClearDepthStencil),
}

/// Filtering mode for image blit operations.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BlitFilter {
    /// Pick nearest texel.
    Nearest = 0,
    /// Take a weighted average of 2x2 texel group.
    Linear = 1,
}

///
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageResolve {
    ///
    pub src_subresource: image::SubresourceLayers,
    ///
    pub src_offset: Offset,
    ///
    pub dst_subresource: image::SubresourceLayers,
    ///
    pub dst_offset: Offset,
    ///
    pub extent: Extent,
}

///
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageBlit {
    ///
    pub src_subresource: image::SubresourceLayers,
    ///
    pub src_bounds: Range<Offset>,
    ///
    pub dst_subresource: image::SubresourceLayers,
    ///
    pub dst_bounds: Range<Offset>,
}

impl<'a, B: Backend, C: Supports<Graphics>, S: Shot, L: Level> CommandBuffer<'a, B, C, S, L> {
    ///
    pub fn begin_render_pass_inline<T>(
        &mut self,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: Rect,
        clear_values: T,
    ) -> RenderPassInlineEncoder<B, L>
    where
        T: IntoIterator,
        T::Item: Borrow<ClearValue>,
    {
        RenderPassInlineEncoder::new(self, render_pass, frame_buffer, render_area, clear_values)
    }

    /// Clear color image
    pub fn clear_color_image(
        &mut self,
        image: &B::Image,
        layout: image::ImageLayout,
        range: image::SubresourceRange,
        value: ClearColor,
    ) {
        self.raw.clear_color_image(image, layout, range, value)
    }

    /// Clear depth-stencil image
    pub fn clear_depth_stencil_image(
        &mut self,
        image: &B::Image,
        layout: image::ImageLayout,
        range: image::SubresourceRange,
        value: ClearDepthStencil,
    ) {
        self.raw.clear_depth_stencil_image(image, layout, range, value)
    }

    /// Bind index buffer view.
    pub fn bind_index_buffer(&mut self, ibv: IndexBufferView<B>) {
        self.raw.bind_index_buffer(ibv)
    }

    /// Bind vertex buffers.
    pub fn bind_vertex_buffers(&mut self, vbs: pso::VertexBufferSet<B>) {
        self.raw.bind_vertex_buffers(vbs)
    }

    /// Bind a graphics pipeline.
    pub fn bind_graphics_pipeline(&mut self, pipeline: &B::GraphicsPipeline) {
        self.raw.bind_graphics_pipeline(pipeline)
    }

    ///
    pub fn bind_graphics_descriptor_sets<T>(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<B::DescriptorSet>,
    {
        self.raw.bind_graphics_descriptor_sets(layout, first_set, sets)
    }

    ///
    pub fn set_viewports<T>(&mut self, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<Viewport>,
    {
        self.raw.set_viewports(viewports)
    }

    ///
    pub fn set_scissors<T>(&mut self, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<Rect>,
    {
        self.raw.set_scissors(scissors)
    }

    ///
    pub fn set_stencil_reference(&mut self, front: StencilValue, back: StencilValue) {
        self.raw.set_stencil_reference(front, back)
    }

    ///
    pub fn set_blend_constants(&mut self, cv: ColorValue) {
        self.raw.set_blend_constants(cv)
    }

    ///
    pub fn push_graphics_constants(&mut self, layout: &B::PipelineLayout, stages: pso::ShaderStageFlags, offset: u32, constants: &[u32]) {
        self.raw.push_graphics_constants(layout, stages, offset, constants)
    }

    ///
    pub fn resolve_image<T>(
        &mut self,
        src: &B::Image,
        src_layout: image::ImageLayout,
        dst: &B::Image,
        dst_layout: image::ImageLayout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageResolve>
    {
        self.raw.resolve_image(src, src_layout, dst, dst_layout, regions)
    }

    ///
    pub fn blit_image<T>(
        &mut self,
        src: &B::Image,
        src_layout: image::ImageLayout,
        dst: &B::Image,
        dst_layout: image::ImageLayout,
        filter: BlitFilter,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageBlit>,
    {
        self.raw.blit_image(src, src_layout, dst, dst_layout, filter, regions)
    }
}

impl<'a, B: Backend, C: Supports<Graphics>, S: Shot> CommandBuffer<'a, B, C, S, Primary> {
    ///
    pub fn begin_render_pass_secondary<T>(
        &mut self,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: Rect,
        clear_values: T,
    ) -> RenderPassSecondaryEncoder<B>
    where
        T: IntoIterator,
        T::Item: Borrow<ClearValue>,
    {
        RenderPassSecondaryEncoder::new(self, render_pass, frame_buffer, render_area, clear_values)
    }
}

impl<'a, B: Backend, C: Supports<GraphicsOrCompute>, S: Shot, L: Level> CommandBuffer<'a, B, C, S, L> {
    ///
    pub fn begin_query(&mut self, query: Query<B>, flags: QueryControl) {
        self.raw.begin_query(query, flags)
    }

    ///
    pub fn end_query(&mut self, query: Query<B>) {
        self.raw.end_query(query)
    }

    ///
    pub fn reset_query_pool(&mut self, pool: &B::QueryPool, queries: Range<QueryId>) {
        self.raw.reset_query_pool(pool, queries)
    }

    ///
    pub fn write_timestamp(&mut self, stage: pso::PipelineStage, query: Query<B>) {
        self.raw.write_timestamp(stage, query)
    }
}
