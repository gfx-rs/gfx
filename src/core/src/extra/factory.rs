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

use device;
use device::{handle, tex};
use device::shade::CreateShaderError;
use render::mesh::{Mesh, VertexFormat};
use render::pso;
use extra::shade::*;

/// Factory extension trait
pub trait FactoryExt<R: device::Resources>: device::Factory<R> {
    /// Create a new mesh from the given vertex data.
    fn create_mesh<T: VertexFormat>(&mut self, data: &[T]) -> Mesh<R> {
        let nv = data.len();
        //debug_assert!(nv <= self.get_capabilities().max_vertex_count);
        let buf = self.create_buffer_static(data, device::BufferRole::Vertex);
        Mesh::from_format(buf, nv as device::VertexCount)
    }

    /// Create a simple program given a vertex shader with a pixel one.
    fn link_program(&mut self, vs_code: &[u8], ps_code: &[u8])
                    -> Result<handle::Program<R>, ProgramError> {

        let vs = match self.create_shader_vertex(vs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };
        let ps = match self.create_shader_pixel(ps_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Pixel(e)),
        };

        let set = device::ShaderSet::Simple(vs, ps);

        self.create_program(&set)
            .map_err(|e| ProgramError::Link(e))
    }

    /// Create a simple program given `ShaderSource` versions of vertex and
    /// pixel shaders, automatically picking available shader variant.
    fn link_program_source(&mut self, vs_src: ShaderSource, ps_src: ShaderSource)
                            -> Result<handle::Program<R>, ProgramError> {
        let model = self.get_capabilities().shader_model;

        match (vs_src.choose(model), ps_src.choose(model)) {
            (Ok(vs_code), Ok(ps_code)) => self.link_program(vs_code, ps_code),
            (Err(_), Ok(_)) => Err(ProgramError::Vertex(CreateShaderError::ModelNotSupported)),
            (_, Err(_)) => Err(ProgramError::Pixel(CreateShaderError::ModelNotSupported)),
        }
    }

    /// Create a strongly-typed Pipeline State.
    fn create_pipeline_state<'a, I: pso::PipelineInit<'a>>(&mut self, init: &I,
                             rasterizer: device::pso::Rasterizer, shaders: &device::ShaderSet<R>)
                             -> Result<pso::PipelineState<R, I::Meta>, device::pso::CreationError>
    {
        use std::collections::HashMap;

        let map = init.declare();
        let mut reg = HashMap::new();
        let topo = rasterizer.topology;
        let raw = try!(self.create_pipeline_state_raw(rasterizer, shaders, &map, &mut reg));
        let meta = init.register(&reg);

        Ok(pso::PipelineState::new(raw, topo, meta))
    }

    /// Create a simple RGBA8 2D texture.
    fn create_texture_rgba8(&mut self, width: u16, height: u16)
                            -> Result<handle::Texture<R>, tex::TextureError> {
        self.create_texture(tex::TextureInfo {
            width: width,
            height: height,
            depth: 1,
            levels: 1,
            kind: tex::Kind::D2,
            format: tex::RGBA8,
        })
    }

    /// Create RGBA8 2D texture with given contents and mipmap chain.
    fn create_texture_rgba8_static(&mut self, width: u16, height: u16, data: &[u32])
                                   -> Result<handle::Texture<R>, tex::TextureError> {
        let info = tex::TextureInfo {
            width: width,
            height: height,
            depth: 1,
            levels: 99,
            kind: tex::Kind::D2,
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
            width: width,
            height: height,
            depth: 0,
            levels: 1,
            kind: tex::Kind::D2,
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

impl<R: device::Resources, F: device::Factory<R>> FactoryExt<R> for F {}
