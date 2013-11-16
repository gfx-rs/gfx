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
#[feature(macro_rules)];

pub mod data;
pub mod interop;

data_manager! {
    data ShaderProgram {
        id: uint,
        ty: uint
    }
}

data_manager! {
    data IndexBuffer {
        id: uint
    }
}

data_manager! {
    data UniformBuffer {
        id: uint
    }
}

data_manager! {
    data VertexBuffer {
        id: uint
    }
}

/// A graphics device manager
pub struct DeviceManager {
    index_buffers: IndexBuffer::Manager,
    vertex_buffers: VertexBuffer::Manager,
    uniform_buffers: UniformBuffer::Manager,
    shader_programs: ShaderProgram::Manager,
}

impl DeviceManager {
    /// Initialise a new graphics device manager
    pub fn new() -> DeviceManager {
        DeviceManager {
            index_buffers: IndexBuffer::Manager::new(),
            vertex_buffers: VertexBuffer::Manager::new(),
            uniform_buffers: UniformBuffer::Manager::new(),
            shader_programs: ShaderProgram::Manager::new(),
        }
    }

    pub fn destroy(self) {}

    pub fn add_vertex_buffer<T>(&mut self, _data: ~[T], _stride: u32) -> VertexBuffer::Handle {
        fail!("Not yet implemented.");
    }

    pub fn destroy_vertex_buffer(&mut self, _handle: VertexBuffer::Handle) {
        fail!("Not yet implemented.");
    }

    pub fn add_index_buffer(&mut self, _data: ~[u32]) -> IndexBuffer::Handle {
        fail!("Not yet implemented.");
    }

    pub fn destroy_index_buffer(&mut self, _handle: IndexBuffer::Handle) {
        fail!("Not yet implemented.");
    }

    pub fn add_uniform_buffer(&mut self/*, ...*/) -> UniformBuffer::Handle {
        fail!("Not yet implemented.");
    }

    pub fn destroy_uniform_buffer(&mut self, _handle: UniformBuffer::Handle) {
        fail!("Not yet implemented.");
    }

    pub fn add_shader_program(&mut self, _shaders: ~[()/*Shader*/]) -> ShaderProgram::Handle {
        fail!("Not yet implemented.");
    }

    pub fn destroy_shader_program(&mut self, _handle: ShaderProgram::Handle) {
        fail!("Not yet implemented.");
    }
}

impl Drop for DeviceManager {
    fn drop(&mut self) {
        // Clean up all the things
    }
}
