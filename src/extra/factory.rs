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
use render::mesh::{Mesh, VertexFormat};
use super::shade::*;

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
    /// Create a simple RGBA8 2D texture.
    fn create_texture_rgba8(&mut self, width: u16, height: u16)
                            -> Result<handle::Texture<R>, tex::TextureError>;
    /// Create RGBA8 2D texture with given contents and mipmap chain.
    fn create_texture_rgba8_static(&mut self, width: u16, height: u16, data: &[u32])
                                   -> Result<handle::Texture<R>, tex::TextureError>;
    /// Create a simple depth+stencil 2D texture.
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

    fn create_texture_rgba8(&mut self, width: u16, height: u16)
                            -> Result<handle::Texture<R>, tex::TextureError> {
        self.create_texture(tex::TextureInfo {
            width: width,
            height: height,
            depth: 1,
            levels: 1,
            kind: tex::TextureKind::Texture2D,
            format: tex::RGBA8,
        })
    }

    fn create_texture_rgba8_static(&mut self, width: u16, height: u16, data: &[u32])
                                   -> Result<handle::Texture<R>, tex::TextureError> {
        let info = tex::TextureInfo {
            width: width,
            height: height,
            depth: 1,
            levels: 99,
            kind: tex::TextureKind::Texture2D,
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
