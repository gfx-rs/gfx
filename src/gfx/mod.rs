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

extern mod gl;

struct HandleManager<T>;

impl<T> HandleManager<T> {
    fn new() -> HandleManager<T> {
        HandleManager;
    }
}

pub struct IndexBuffer;
pub struct VertexBuffer;
pub struct UniformBuffer;

pub struct Shader;
pub struct ShaderProgram;

/// A graphics device manager
pub struct DeviceManager {
    index_buffers: HandleManager<IndexBuffer>,
    vertex_buffers: HandleManager<VertexBuffer>,
    uniform_buffers: HandleManager<UniformBuffer>,
    shader_programs: HandleManager<ShaderProgram>,
}

impl DeviceManager {
    /// Initialise a new graphics device manager
    pub fn new() -> DeviceManager {
        DeviceManager {
            index_buffers: HandleManager::new(),
            vertex_buffers: HandleManager::new(),
            uniform_buffers: HandleManager::new(),
            shader_programs: HandleManager::new(),
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

pub type Points = ~[u32];
pub type Lines = ~[[u32,..2]];
pub type Triangles = ~[[u32,..3]];

pub type Vertex2<T> = [T,..2];
pub type Vertex3<T> = [T,..3];
pub type Vertex4<T> = [T,..4];
pub type Matrix2x2<T> = [[T,..2],..2];
pub type Matrix2x3<T> = [[T,..3],..2];
pub type Matrix2x4<T> = [[T,..4],..2];
pub type Matrix3x2<T> = [[T,..2],..3];
pub type Matrix3x3<T> = [[T,..3],..3];
pub type Matrix3x4<T> = [[T,..4],..3];
pub type Matrix4x2<T> = [[T,..2],..4];
pub type Matrix4x3<T> = [[T,..3],..4];
pub type Matrix4x4<T> = [[T,..4],..4];
