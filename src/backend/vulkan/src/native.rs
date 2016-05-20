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

use std::ffi::CStr;
use std::{cell, fmt, hash};
use vk;

//Clone + Hash + Debug + Eq + PartialEq + Any + Send + Sync;


pub struct Shader(pub vk::PipelineShaderStageCreateInfo);

impl Clone for Shader {
    fn clone(&self) -> Shader {
        Shader(vk::PipelineShaderStageCreateInfo {
            .. self.0
        })
    }
}

impl fmt::Debug for Shader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = unsafe { CStr::from_ptr(self.0.pName) }.to_str().unwrap();
        write!(f, "Shader({}, {}, {})", self.0.stage, name, self.0.module)
    }
}

impl hash::Hash for Shader {
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
        self.0.stage.hash(state);
        //self.0.pName.hash(state);
        self.0.module.hash(state);
    }
}

impl PartialEq for Shader {
    fn eq(&self, other: &Shader) -> bool {
        self.0.stage == other.0.stage &&
        self.0.module == other.0.module
    }
}

impl Eq for Shader {}
unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}


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

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct Pipeline {
    pub pipeline: vk::Pipeline,
    pub pipe_layout: vk::PipelineLayout,
    pub desc_layout: vk::DescriptorSetLayout,
    pub desc_pool: vk::DescriptorPool,
}
