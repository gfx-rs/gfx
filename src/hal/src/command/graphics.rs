use std::ops::Range;

use Backend;
use pso;
use buffer::IndexBufferView;
use image::{ImageLayout, SubresourceRange};
use queue::capability::{Graphics, Supports};
use super::{CommandBuffer, RawCommandBuffer, RenderPassInlineEncoder};


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

impl<'a, B: Backend, C: Supports<Graphics>> CommandBuffer<'a, B, C> {
    ///
    pub fn begin_renderpass_inline(
        &mut self,
        render_pass: &B::RenderPass,
        frame_buffer: &B::Framebuffer,
        render_area: Rect,
        clear_values: &[ClearValue],
    ) -> RenderPassInlineEncoder<B>
    {
        RenderPassInlineEncoder::new(self, render_pass, frame_buffer, render_area, clear_values)
    }

    /// Clear color image
    pub fn clear_color_image(
        &mut self,
        image: &B::Image,
        layout: ImageLayout,
        range: SubresourceRange,
        value: ClearColor,
    ) {
        self.raw.clear_color_image(image, layout, range, value)
    }

    /// Clear depth-stencil image
    pub fn clear_depth_stencil_image(
        &mut self,
        image: &B::Image,
        layout: ImageLayout,
        range: SubresourceRange,
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
    pub fn bind_graphics_descriptor_sets(
        &mut self,
        layout: &B::PipelineLayout,
        first_set: usize,
        sets: &[&B::DescriptorSet],
    ) {
        self.raw.bind_graphics_descriptor_sets(layout, first_set, sets)
    }

    ///
    pub fn set_viewports(&mut self, viewports: &[Viewport]) {
        self.raw.set_viewports(viewports)
    }

    ///
    pub fn set_scissors(&mut self, scissors: &[Rect]) {
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
}
