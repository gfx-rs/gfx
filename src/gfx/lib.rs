// Copyright 2013 The Gfx-rs Developers.
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

#[feature(globs)];

// extern mod gl;
pub mod interop;

#[cfg(test)] use lib::resource::{Handle, ResourceManager};
#[cfg(not(test))] use resource::{Handle, ResourceManager};

mod resource;

pub struct IndexBuffer;
pub struct VertexBuffer;
pub struct UniformBuffer;

pub struct Shader;
pub struct ShaderProgram;

/// A graphics device manager
pub struct DeviceManager {
    index_buffers: ResourceManager<IndexBuffer>,
    vertex_buffers: ResourceManager<VertexBuffer>,
    uniform_buffers: ResourceManager<UniformBuffer>,
    shader_programs: ResourceManager<ShaderProgram>,
}

impl DeviceManager {
    /// Initialise a new graphics device manager
    pub fn new() -> DeviceManager {
        DeviceManager {
            index_buffers: ResourceManager::new(),
            vertex_buffers: ResourceManager::new(),
            uniform_buffers: ResourceManager::new(),
            shader_programs: ResourceManager::new(),
        }
    }

    pub fn destroy(self) {}

    pub fn add_vertex_buffer<T>(&mut self, _data: ~[T], _stride: u32) -> Handle<VertexBuffer> {
        fail!("Not yet implemented.");
    }

    pub fn destroy_vertex_buffer(&mut self, _handle: Handle<VertexBuffer>) {
        fail!("Not yet implemented.");
    }

    pub fn add_index_buffer(&mut self, _data: ~[u32]) -> Handle<IndexBuffer> {
        fail!("Not yet implemented.");
    }

    pub fn destroy_index_buffer(&mut self, _handle: Handle<IndexBuffer>) {
        fail!("Not yet implemented.");
    }

    pub fn add_uniform_buffer(&mut self/*, ...*/) -> Handle<UniformBuffer> {
        fail!("Not yet implemented.");
    }

    pub fn destroy_uniform_buffer(&mut self, _handle: Handle<UniformBuffer>) {
        fail!("Not yet implemented.");
    }

    pub fn add_shader_program(&mut self, _shaders: ~[Shader]) -> Handle<ShaderProgram> {
        fail!("Not yet implemented.");
    }

    pub fn destroy_shader_program(&mut self, _handle: Handle<ShaderProgram>) {
        fail!("Not yet implemented.");
    }
}

impl Drop for DeviceManager {
    fn drop(&mut self) {
        // Clean up all the things
    }
}
