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

use device::shade::UniformValue;

pub type BlockVar = u8;
pub type UniformVar = u16;
pub type TextureVar = u8;


/// Environment storage structure, contains a set of parameters
/// to be provided for shader programs
pub struct Storage {
    blocks: Vec<(String, super::BufferHandle)>,
    uniforms: Vec<(String, UniformValue)>,
    textures: Vec<(String, super::TextureHandle, super::SamplerHandle)>,
}

impl Storage {
    pub fn new() -> Storage {
    	Storage {
    		blocks: Vec::new(),
    		uniforms: Vec::new(),
    		textures: Vec::new(),
    	}
    }

    // expansion methods

    pub fn add_block(&mut self, name: &str, buf: super::BufferHandle) -> BlockVar {
    	self.blocks.push((name.to_string(), buf));
    	(self.blocks.len() - 1) as BlockVar
    }

    pub fn add_uniform(&mut self, name: &str, value: UniformValue) -> UniformVar {
    	self.uniforms.push((name.to_string(), value));
    	(self.uniforms.len() - 1) as UniformVar
    }

    pub fn add_texture(&mut self, name: &str, texture: super::TextureHandle, sampler: super::SamplerHandle) -> TextureVar {
    	self.textures.push((name.to_string(), texture, sampler));
    	(self.textures.len() - 1) as TextureVar
    }

    // mutation methods

    pub fn set_block(&mut self, var: BlockVar, buf: super::BufferHandle) {
		let &(_, ref mut block) = self.blocks.get_mut(var as uint);
        *block = buf;
    }
    
    pub fn set_uniform(&mut self, var: UniformVar, value: UniformValue) {
    	let &(_, ref mut uniform) = self.uniforms.get_mut(var as uint);
        *uniform = value;
    }

    pub fn set_texture(&mut self, var: TextureVar, texture: super::TextureHandle, sampler: super::SamplerHandle) {
        let &(_, ref mut tex, ref mut sam) = self.textures.get_mut(var as uint);
        *tex = texture;
        *sam = sampler;    	
    }
}
