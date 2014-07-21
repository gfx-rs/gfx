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

use device::shade::{UniformValue, ValueI32, ProgramMeta};

pub type BlockVarId = u8;
pub type UniformVarId = u16;
pub type TextureVarId = u8;

pub trait ParameterSink {
    fn find_block  (&self, name: &str) -> Option<BlockVarId>;
    fn find_uniform(&self, name: &str) -> Option<UniformVarId>;
    fn find_texture(&self, name: &str) -> Option<TextureVarId>;
}

//impl ParameterSink for ProgramMeta

pub trait Uploader {
    fn set_block  (&mut self, BlockVarId, super::BufferHandle);
    fn set_uniform(&mut self, UniformVarId, UniformValue);
    fn set_texture(&mut self, TextureVarId, super::TextureHandle);
}

pub trait ToUniform {
    fn to_uniform(&self) -> UniformValue;
}

impl ToUniform for i32 {
    fn to_uniform(&self) -> UniformValue {
        ValueI32(*self)
    }
}

#[deriving(Clone, Show)]
pub enum ParameterLinkError<'a> {
    LinkBadProgram,
    LinkInternalError,
    LinkMissingBlock(&'a str),
    LinkMissingUniform(&'a str),
    LinkMissingTexture(&'a str),
}

pub trait ShaderParam<L> {
    fn create_link<S: ParameterSink>(&S) -> Result<L, ParameterLinkError<'static>>;
    fn upload<U: Uploader>(&self, &L, &mut U);
}

pub struct ShaderBundle<T, L> {
    /// Shader program
    program: super::ProgramHandle,
    /// Global data in a user-provided struct
    pub data: T,
    /// Hidden link that provides parameter indices for user data
    link: L,
}