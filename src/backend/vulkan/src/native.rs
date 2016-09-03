// Copyright 2016 The Gfx-rs Developers.
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

use std::{cell, hash};
use vk;
use core;
use Resources as R;
use mirror::SpirvReflection;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Shader {
    pub shader: vk::ShaderModule,
    pub reflection: SpirvReflection,
}
unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Program {
    pub vertex: vk::ShaderModule,
    pub geometry: Option<vk::ShaderModule>,
    pub pixel: vk::ShaderModule,
}
unsafe impl Send for Program {}
unsafe impl Sync for Program {}


#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Buffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
}
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Texture {
    pub image: vk::Image,
    pub layout: cell::Cell<vk::ImageLayout>,
    pub memory: vk::DeviceMemory,
}
impl hash::Hash for Texture {
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
        self.image.hash(state);
        self.layout.get().hash(state);
        self.memory.hash(state);
    }
}
unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct TextureView {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub layout: vk::ImageLayout,
    pub sub_range: vk::ImageSubresourceRange,
}
unsafe impl Send for TextureView {}
unsafe impl Sync for TextureView {}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Pipeline {
    pub pipeline: vk::Pipeline,
    pub pipe_layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
    pub desc_pool: vk::DescriptorPool,
    pub render_pass: vk::RenderPass,
    pub program: core::handle::Program<R>,
}
