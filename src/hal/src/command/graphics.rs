//! `CommandBuffer` methods for graphics operations.
use std::borrow::Borrow;
use std::ops::Range;

use Backend;
use {image, pso};
use buffer::IndexBufferView;
use query::{Query, QueryControl, QueryId};
use queue::capability::{Graphics, GraphicsOrCompute, Supports};
use super::{
    CommandBuffer, RawCommandBuffer,
    RenderPassInlineEncoder, RenderPassSecondaryEncoder,
    Shot, Level, Primary,
    ClearColorRaw, ClearDepthStencilRaw, ClearValueRaw, DescriptorSetOffset,
};


/// A universal clear color supporting integer formats
/// as well as the standard floating-point.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ClearColor {
    /// Standard floating-point `vec4` color
    Float(pso::ColorValue),
    /// Integer vector to clear `ivec4` targets.
    Int([i32; 4]),
    /// Unsigned int vector to clear `uvec4` targets.
    Uint([u32; 4]),
}

macro_rules! impl_clear {
    { $( $ty:ty = $sub:ident[$a:expr, $b:expr, $c:expr, $d:expr], )* } => {
        $(
            impl From<$ty> for ClearColor {
                fn from(v: $ty) -> Self {
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
    fn from(v: f32) -> Self {
        ClearColor::Float([v, 0.0, 0.0, 0.0])
    }
}
impl From<i32> for ClearColor {
    fn from(v: i32) -> Self {
        ClearColor::Int([v, 0, 0, 0])
    }
}
impl From<u32> for ClearColor {
    fn from(v: u32) -> Self {
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
pub struct ClearDepthStencil(pub pso::DepthValue, pub pso::StencilValue);

impl From<ClearDepthStencil> for ClearDepthStencilRaw {
    fn from(value: ClearDepthStencil) -> Self {
        ClearDepthStencilRaw {
            depth: value.0,
            stencil: value.1,
        }
    }
}

/// General clear values for attachments (color or depth-stencil).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ClearValue {
    ///
    Color(ClearColor),
    ///
    DepthStencil(ClearDepthStencil),
}

impl From<ClearValue> for ClearValueRaw {
    fn from(value: ClearValue) -> Self {
        match value {
            ClearValue::Color(color) => ClearValueRaw { color: color.into() },
            ClearValue::DepthStencil(ds) => ClearValueRaw { depth_stencil: ds.into() },
        }
    }
}

/// Attachment clear description for the current subpass.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AttachmentClear {
    /// Clear color attachment.
    Color {
        /// Index inside the `SubpassDesc::colors` array.
        index: usize,
        /// Value to clear with.
        value: ClearColor,
    },
    /// Clear depth-stencil attachment.
    DepthStencil {
        /// Depth value to clear with.
        depth: Option<pso::DepthValue>,
        /// Stencil value to clear with.
        stencil: Option<pso::StencilValue>,
    },
}

/// Parameters for an image resolve operation,
/// where a multi-sampled image is copied into a single-sampled
/// image.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageResolve {
    /// Source image and layers.
    pub src_subresource: image::SubresourceLayers,
    /// Source image offset.
    pub src_offset: image::Offset,
    /// Destination image and layers.
    pub dst_subresource: image::SubresourceLayers,
    /// Destination image offset.
    pub dst_offset: image::Offset,
    /// Image extent.
    pub extent: image::Extent,
}

/// Parameters for an image blit operation, where a portion of one image
/// is copied into another, possibly with scaling and filtering.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageBlit {
    /// Source image and layers.
    pub src_subresource: image::SubresourceLayers,
    /// Source image bounds.
    pub src_bounds: Range<image::Offset>,
    /// Destination image and layers.
    pub dst_subresource: image::SubresourceLayers,
    /// Destination image bounds.
    pub dst_bounds: Range<image::Offset>,
}

impl<'a, B: Backend, C: Supports<Graphics>, S: Shot, L: Level> CommandBuffer<'a, B, C, S, L> {
    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn clear_image<T>(
        &mut self,
        image: &B::Image,
        layout: image::Layout,
        color: ClearColor,
        depth_stencil: ClearDepthStencil,
        subresource_ranges: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<image::SubresourceRange>,
    {
        self.raw.clear_image(image, layout, color.into(), depth_stencil.into(), subresource_ranges)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn bind_index_buffer(&mut self, ibv: IndexBufferView<B>) {
        self.raw.bind_index_buffer(ibv)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn bind_vertex_buffers(&mut self, first_binding: u32, vbs: pso::VertexBufferSet<B>) {
        self.raw.bind_vertex_buffers(first_binding, vbs)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn bind_graphics_pipeline(&mut self, pipeline: &B::GraphicsPipeline) {
        self.raw.bind_graphics_pipeline(pipeline)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn bind_graphics_descriptor_sets<I, J>(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: I,
        offsets: J,
    ) where
        I: IntoIterator,
        I::Item: Borrow<B::DescriptorSet>,
        J: IntoIterator,
        J::Item: Borrow<DescriptorSetOffset>,
    {
        self.raw.bind_graphics_descriptor_sets(layout, first_set, sets, offsets)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn set_viewports<T>(&mut self, first_viewport: u32, viewports: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Viewport>,
    {
        self.raw.set_viewports(first_viewport, viewports)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn set_scissors<T>(&mut self, first_scissor: u32, scissors: T)
    where
        T: IntoIterator,
        T::Item: Borrow<pso::Rect>,
    {
        self.raw.set_scissors(first_scissor, scissors)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn set_stencil_reference(&mut self, faces: pso::Face, value: pso::StencilValue) {
        self.raw.set_stencil_reference(faces, value);
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn set_stencil_read_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        self.raw.set_stencil_read_mask(faces, value);
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn set_stencil_write_mask(&mut self, faces: pso::Face, value: pso::StencilValue) {
        self.raw.set_stencil_write_mask(faces, value);
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn set_blend_constants(&mut self, cv: pso::ColorValue) {
        self.raw.set_blend_constants(cv)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn set_depth_bounds(&mut self, bounds: Range<f32>) {
        self.raw.set_depth_bounds(bounds)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn set_line_width(&mut self, width: f32) {
        self.raw.set_line_width(width);
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn set_depth_bias(&mut self, depth_bias: pso::DepthBias) {
        self.raw.set_depth_bias(depth_bias);
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn push_graphics_constants(&mut self, layout: &B::PipelineLayout, stages: pso::ShaderStageFlags, offset: u32, constants: &[u32]) {
        self.raw.push_graphics_constants(layout, stages, offset, constants)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn resolve_image<T>(
        &mut self,
        src: &B::Image,
        src_layout: image::Layout,
        dst: &B::Image,
        dst_layout: image::Layout,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageResolve>
    {
        self.raw.resolve_image(src, src_layout, dst, dst_layout, regions)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn blit_image<T>(
        &mut self,
        src: &B::Image,
        src_layout: image::Layout,
        dst: &B::Image,
        dst_layout: image::Layout,
        filter: image::Filter,
        regions: T,
    ) where
        T: IntoIterator,
        T::Item: Borrow<ImageBlit>,
    {
        self.raw.blit_image(src, src_layout, dst, dst_layout, filter, regions)
    }
}

impl<'a, B: Backend, C: Supports<Graphics>, S: Shot> CommandBuffer<'a, B, C, S, Primary> {
    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn begin_render_pass_inline<T>(
        &mut self,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: pso::Rect,
        clear_values: T,
    ) -> RenderPassInlineEncoder<B, Primary>
    where
        T: IntoIterator,
        T::Item: Borrow<ClearValue>,
    {
        RenderPassInlineEncoder::new(self, render_pass, frame_buffer, render_area, clear_values)
    }

    /// Creates a new secondary render pass.
    pub fn begin_render_pass_secondary<T>(
        &mut self,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: pso::Rect,
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
    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn begin_query(&mut self, query: Query<B>, flags: QueryControl) {
        self.raw.begin_query(query, flags)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn end_query(&mut self, query: Query<B>) {
        self.raw.end_query(query)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn reset_query_pool(&mut self, pool: &B::QueryPool, queries: Range<QueryId>) {
        self.raw.reset_query_pool(pool, queries)
    }

    /// Identical to the `RawCommandBuffer` method of the same name.
    pub fn write_timestamp(&mut self, stage: pso::PipelineStage, query: Query<B>) {
        self.raw.write_timestamp(stage, query)
    }
}
