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

//! Factory extension. Provides resource construction shortcuts.

use gfx_core::{format, handle, tex};
use gfx_core::{Primitive, Resources, ShaderSet, VertexCount};
use gfx_core::factory::{BufferRole, Factory};
use gfx_core::pso::{CreationError, Descriptor};
use gfx_core::state::{CullFace, Rasterizer};
use encoder::Encoder;
use mesh::{Mesh, Slice, SliceKind, ToIndexSlice, VertexFormat};
use pso;
use extra::shade::{ProgramError, ShaderSource};

/// Error creating a PipelineState
#[derive(Clone, PartialEq, Debug)]
pub enum PipelineStateError {
    /// Shader program failed to link.
    Program(ProgramError),
    /// Unable to create PSO descriptor due to mismatched formats.
    DescriptorInit(pso::InitError),
    /// Device failed to create the handle give the descriptor.
    DeviceCreate(CreationError),
}


/// Factory extension trait
pub trait FactoryExt<R: Resources>: Factory<R> + Sized {
    /// Create a new graphics command Encoder
    fn create_encoder(&mut self) -> Encoder<R, Self::CommandBuffer> {
        Encoder::create(self)
    }

    /// Create a new mesh from the given vertex data.
    fn create_mesh<T: VertexFormat>(&mut self, data: &[T]) -> Mesh<R> {
        let nv = data.len();
        //debug_assert!(nv <= self.get_capabilities().max_vertex_count);
        let buf = self.create_buffer_static(data, BufferRole::Vertex);
        Mesh::from_format(buf, nv as VertexCount)
    }

    /// Create a vertex buffer with an associated slice.
    fn create_vertex_buffer<T>(&mut self, data: &[T])
                            -> (handle::Buffer<R, T>, Slice<R>) where
                            T: pso::Structure<format::Format>
    {
        let nv = data.len();
        //debug_assert!(nv <= self.get_capabilities().max_vertex_count);
        let buf = self.create_buffer_static(data, BufferRole::Vertex);
        (buf, Slice {
            start: 0,
            end: nv as VertexCount,
            instances: None,
            kind: SliceKind::Vertex,
        })
    }

    /// Create a vertex buffer with an index, returned by a slice.
    fn create_vertex_buffer_indexed<V, I>(&mut self, vd: &[V], id: I)
                                    -> (handle::Buffer<R, V>, Slice<R>) where
        V: pso::Structure<format::Format>,
        I: ToIndexSlice<R>,
    {
        let buf = self.create_buffer_static(vd, BufferRole::Vertex);
        (buf, id.to_slice(self))
    }

    /// Create a constant buffer for `num` identical elements of type `T`.
    fn create_constant_buffer<T>(&mut self, num: usize) -> handle::Buffer<R, T> {
        self.create_buffer_dynamic(num, BufferRole::Uniform)
    }

    /// Create a shader set from a given vs/ps code for multiple shader models.
    fn create_shader_set(&mut self, vs_code: &[u8], ps_code: &[u8])
                         -> Result<ShaderSet<R>, ProgramError> {
        let vs = match self.create_shader_vertex(vs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };
        let ps = match self.create_shader_pixel(ps_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Pixel(e)),
        };
        Ok(ShaderSet::Simple(vs, ps))
    }

    /// Create a simple program given a vertex shader with a pixel one.
    fn link_program(&mut self, vs_code: &[u8], ps_code: &[u8])
                    -> Result<handle::Program<R>, ProgramError> {

        let set = try!(self.create_shader_set(vs_code, ps_code));
        self.create_program(&set)
            .map_err(|e| ProgramError::Link(e))
    }

    /// Create a simple program given `ShaderSource` versions of vertex and
    /// pixel shaders, automatically picking available shader variant.
    fn link_program_source(&mut self, vs_src: ShaderSource, ps_src: ShaderSource)
                           -> Result<handle::Program<R>, ProgramError> {
        use gfx_core::shade::CreateShaderError;
        let model = self.get_capabilities().shader_model;

        match (vs_src.choose(model), ps_src.choose(model)) {
            (Ok(vs_code), Ok(ps_code)) => self.link_program(vs_code, ps_code),
            (Err(_), Ok(_)) => Err(ProgramError::Vertex(CreateShaderError::ModelNotSupported)),
            (_, Err(_)) => Err(ProgramError::Pixel(CreateShaderError::ModelNotSupported)),
        }
    }

    /// Create a strongly-typed Pipeline State.
    fn create_pipeline_state<I: pso::PipelineInit>(&mut self, shaders: &ShaderSet<R>,
                             primitive: Primitive, rasterizer: Rasterizer, init: &I)
                             -> Result<pso::PipelineState<R, I::Meta>, PipelineStateError>
    {
        let program = match self.create_program(shaders) {
            Ok(p) => p,
            Err(e) => return Err(PipelineStateError::Program(ProgramError::Link(e))),
        };
        let mut descriptor = Descriptor::new(primitive, rasterizer);
        let meta = match init.link_to(&mut descriptor, program.get_info()) {
            Ok(m) => m,
            Err(e) => return Err(PipelineStateError::DescriptorInit(e)),
        };
        let raw = match self.create_pipeline_state_raw(&program, &descriptor) {
            Ok(raw) => raw,
            Err(e) => return Err(PipelineStateError::DeviceCreate(e)),
        };

        Ok(pso::PipelineState::new(raw, primitive, meta))
    }

    /// Create a simplified version of the Pipeline State,
    /// which works on triangles, and only has VS and PS shaders in it.
    fn create_pipeline_simple<I: pso::PipelineInit>(&mut self, vs: &[u8], ps: &[u8], cull: CullFace, init: &I)
                              -> Result<pso::PipelineState<R, I::Meta>, PipelineStateError>
    {
        match self.create_shader_set(vs, ps) {
            Ok(ref s) => self.create_pipeline_state(s,
                Primitive::TriangleList, Rasterizer::new_fill(cull), init),
            Err(e) => Err(PipelineStateError::Program(e)),
        }
    }

    /// Create a simple RGBA8 2D texture.
    fn create_texture_rgba8(&mut self, width: u16, height: u16)
                            -> Result<handle::Texture<R>, tex::TextureError> {
        self.create_texture(tex::TextureInfo {
            kind: tex::Kind::D2(width, height, tex::AaMode::Single),
            levels: 1,
            format: tex::RGBA8,
        })
    }

    /// Create RGBA8 2D texture with given contents and mipmap chain.
    fn create_texture_rgba8_static(&mut self, width: u16, height: u16, data: &[u32])
                                   -> Result<handle::Texture<R>, tex::TextureError> {
        let info = tex::TextureInfo {
            kind: tex::Kind::D2(width, height, tex::AaMode::Single),
            levels: 99,
            format: tex::RGBA8,
        };
        match self.create_texture_static(info, data) {
            Ok(handle) => {
                self.generate_mipmap(&handle);
                Ok(handle)
            },
            Err(e) => Err(e),
        }
    }

    /// Create a simple depth+stencil 2D texture.
    fn create_texture_depth_stencil(&mut self, width: u16, height: u16)
                                    -> Result<handle::Texture<R>, tex::TextureError> {
        self.create_texture(tex::TextureInfo {
            kind: tex::Kind::D2(width, height, tex::AaMode::Single),
            levels: 1,
            format: tex::Format::DEPTH24_STENCIL8,
        })
    }

    /// Create a linear sampler with clamping to border.
    fn create_sampler_linear(&mut self) -> handle::Sampler<R> {
        self.create_sampler(tex::SamplerInfo::new(
            tex::FilterMethod::Trilinear,
            tex::WrapMode::Clamp,
        ))
    }
}

impl<R: Resources, F: Factory<R>> FactoryExt<R> for F {}
