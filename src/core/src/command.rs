// Copyright 2014 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Command Buffer device interface

use {Resources, IndexType, InstanceCount, VertexCount};
use {state, target, pso, shade, texture};

/// A universal clear color supporting integet formats
/// as well as the standard floating-point.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub enum ClearColor {
    /// Standard floating-point vec4 color
    Float([f32; 4]),
    /// Integer vector to clear ivec4 targets.
    Int([i32; 4]),
    /// Unsigned int vector to clear uvec4 targets.
    Uint([u32; 4]),
}

/// Optional instance parameters: (instance count, buffer offset)
pub type InstanceParams = (InstanceCount, VertexCount);

/// An interface of the abstract command buffer. It collects commands in an
/// efficient API-specific manner, to be ready for execution on the device.
#[allow(missing_docs)]
pub trait Buffer<R: Resources> {
    /// Reset the command buffer contents, retain the allocated storage
    fn reset(&mut self);
    /// Bind a pipeline state object
    fn bind_pipeline_state(&mut self, R::PipelineStateObject);
    /// Bind a complete set of vertex buffers
    fn bind_vertex_buffers(&mut self, pso::VertexBufferSet<R>);
    /// Bind a complete set of constant buffers
    fn bind_constant_buffers(&mut self, &[pso::ConstantBufferParam<R>]);
    /// Bind a global constant
    fn bind_global_constant(&mut self, shade::Location, shade::UniformValue);
    /// Bind a complete set of shader resource views
    fn bind_resource_views(&mut self, &[pso::ResourceViewParam<R>]);
    /// Bind a complete set of unordered access views
    fn bind_unordered_views(&mut self, &[pso::UnorderedViewParam<R>]);
    /// Bind a complete set of samplers
    fn bind_samplers(&mut self, &[pso::SamplerParam<R>]);
    /// Bind a complete set of pixel targets, including multiple
    /// colors views and an optional depth/stencil view.
    fn bind_pixel_targets(&mut self, pso::PixelTargetSet<R>);
    /// Bind an index buffer
    fn bind_index(&mut self, R::Buffer, IndexType);
    /// Set scissor rectangle
    fn set_scissor(&mut self, target::Rect);
    /// Set reference values for the blending and stencil front/back
    fn set_ref_values(&mut self, state::RefValues);
    /// Update a vertex/index/uniform buffer
    fn update_buffer(&mut self, R::Buffer, data: &[u8], offset: usize);
    /// Update a texture
    fn update_texture(&mut self, R::Texture, texture::Kind, Option<texture::CubeFace>,
                      data: &[u8], texture::RawImageInfo);
    fn generate_mipmap(&mut self, R::ShaderResourceView);
    /// Clear color target
    fn clear_color(&mut self, R::RenderTargetView, ClearColor);
    fn clear_depth_stencil(&mut self, R::DepthStencilView,
                           Option<target::Depth>, Option<target::Stencil>);
    /// Draw a primitive
    fn call_draw(&mut self, VertexCount, VertexCount, Option<InstanceParams>);
    /// Draw a primitive with index buffer
    fn call_draw_indexed(&mut self, VertexCount, VertexCount, VertexCount, Option<InstanceParams>);
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
