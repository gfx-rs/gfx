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
use device::shade::{Stage, CreateShaderError};
use render::{Renderer, RenderState, ParamStorage};
use render::mesh::{Mesh, VertexFormat};
use super::shade::*;

/// Factory extension that allows creating new renderers.
pub trait RenderFactory<R: device::Resources, C: device::draw::CommandBuffer<R>> {
    /// Create a new renderer
    fn create_renderer(&mut self) -> Renderer<R, C>;
}

impl<
    R: device::Resources,
    C: device::draw::CommandBuffer<R>,
    F: device::Factory<R>,
> RenderFactory<R, C> for F {
    fn create_renderer(&mut self) -> Renderer<R, C> {
        Renderer {
            command_buffer: device::draw::CommandBuffer::new(),
            data_buffer: device::draw::DataBuffer::new(),
            handles: handle::Manager::new(),
            common_array_buffer: self.create_array_buffer(),
            draw_frame_buffer: self.create_frame_buffer(),
            read_frame_buffer: self.create_frame_buffer(),
            render_state: RenderState::new(),
            parameters: ParamStorage::new(),
        }
    }
}

/// Factory extension trait
pub trait FactoryExt<R: device::Resources> {
    /// Create a new mesh from the given vertex data.
    /// Convenience function around `create_buffer` and `Mesh::from_format`.
    fn create_mesh<T: VertexFormat + Copy>(&mut self, data: &[T]) -> Mesh<R>;
    /// Create a simple program given a vertex shader with a fragment one.
    fn link_program(&mut self, vs_code: &[u8], fs_code: &[u8])
                    -> Result<handle::Program<R>, ProgramError>;
    /// Create a simple program given `ShaderSource` versions of vertex and
    /// fragment shaders, chooss the matching versions for the device.
    fn link_program_source(&mut self, vs_src: ShaderSource, fs_src: ShaderSource,
                           caps: &device::Capabilities)
                           -> Result<handle::Program<R>, ProgramError>;
    /// Create a simple RGBA8 2D texture
    fn crate_texture_rgba8(&mut self, width: u16, height: u16, mipmap: bool)
                           -> Result<handle::Texture<R>, tex::TextureError>;
    /// Create a simple depth+stencil 2D texture
    fn create_texture_depth_stencil(&mut self, width: u16, height: u16)
                                    -> Result<handle::Texture<R>, tex::TextureError>;
}

impl<R: device::Resources, F: device::Factory<R>> FactoryExt<R> for F {
    fn create_mesh<T: VertexFormat + Copy>(&mut self, data: &[T]) -> Mesh<R> {
        let nv = data.len();
        //debug_assert!(nv < self.max_vertex_count); //TODO
        let buf = self.create_buffer_static(data);
        Mesh::from_format(buf, nv as device::VertexCount)
    }

    fn link_program(&mut self, vs_code: &[u8], fs_code: &[u8])
                    -> Result<handle::Program<R>, ProgramError> {
        let vs = match self.create_shader(Stage::Vertex, vs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };
        let fs = match self.create_shader(Stage::Fragment, fs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Fragment(e)),
        };

        self.create_program(&[vs, fs], None)
            .map_err(|e| ProgramError::Link(e))
    }

    fn link_program_source(&mut self, vs_src: ShaderSource, fs_src: ShaderSource,
                           caps: &device::Capabilities)
                           -> Result<handle::Program<R>, ProgramError> {
        let model = caps.shader_model;
        let err_model = CreateShaderError::ModelNotSupported;

        let vs = match vs_src.choose(model) {
            Ok(code) => match self.create_shader(Stage::Vertex, code) {
                Ok(s) => s,
                Err(e) => return Err(ProgramError::Vertex(e)),
            },
            Err(_) => return Err(ProgramError::Vertex(err_model))
        };

        let fs = match fs_src.choose(model) {
            Ok(code) => match self.create_shader(Stage::Fragment, code) {
                Ok(s) => s,
                Err(e) => return Err(ProgramError::Fragment(e)),
            },
            Err(_) => return Err(ProgramError::Fragment(err_model))
        };

        self.create_program(&[vs, fs], Some(fs_src.targets))
            .map_err(|e| ProgramError::Link(e))
    }

    fn crate_texture_rgba8(&mut self, width: u16, height: u16, mipmap: bool)
                           -> Result<handle::Texture<R>, tex::TextureError> {
        self.create_texture(tex::TextureInfo {
            width: width,
            height: height,
            depth: 0,
            levels: if mipmap {99} else {1},
            kind: tex::TextureKind::Texture2D,
            format: tex::RGBA8,
        })
    }

    fn create_texture_depth_stencil(&mut self, width: u16, height: u16)
                                    -> Result<handle::Texture<R>, tex::TextureError> {
        self.create_texture(tex::TextureInfo {
            width: width,
            height: height,
            depth: 0,
            levels: 1,
            kind: tex::TextureKind::Texture2D,
            format: tex::Format::DEPTH24_STENCIL8,
        })
    }
}
