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

use device::shade::{UniformValue, ProgramMeta};

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

    // accessors

    pub fn get_block(&self, var: BlockVar) -> super::BufferHandle {
        let &(_, buf) = self.blocks.get(var as uint);
        buf
    }

    pub fn get_uniform(&self, var: UniformVar) -> UniformValue {
        let &(_, value) = self.uniforms.get(var as uint);
        value
    }

    pub fn get_texture(&self, var: TextureVar) -> (super::TextureHandle, super::SamplerHandle) {
        let &(_, texture, sampler) = self.textures.get(var as uint);
        (texture, sampler)
    }
}


/// Environment shortcut - the acceleration structure used for
/// binding shader program parameters. Each *Var serves as a
/// pointer from a program parameter to the environment data.
pub struct Shortcut {
    pub blocks: Vec<BlockVar>,
    pub uniforms: Vec<UniformVar>,
    pub textures: Vec<TextureVar>,
}

impl Shortcut {
    pub fn is_fit(&self, program: &ProgramMeta) -> bool {
        self.blocks.len() == program.blocks.len() &&
        self.uniforms.len() == program.uniforms.len() &&
        self.textures.len() == program.textures.len()
    }

    pub fn build(storage: &Storage, program: &ProgramMeta) -> Result<Shortcut,()> {
        let sh = Shortcut {
            blocks: program.blocks.iter().scan((), |_, b|
                storage.blocks.iter().position(|&(ref name,_)| name==&b.name).map(|p| p as BlockVar)
                ).collect(),
            uniforms: program.uniforms.iter().scan((), |_, u|
                storage.uniforms.iter().position(|&(ref name, _)| name==&u.name).map(|p| p as UniformVar)
                ).collect(),
            textures: program.textures.iter().scan((), |_, t|
                storage.textures.iter().position(|&(ref name, _, _)| name==&t.name).map(|p| p as TextureVar)
                ).collect(),
        };
        if sh.is_fit(program) {Ok(sh)}
        else {Err(())}
    }
}
