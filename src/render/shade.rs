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

use std::mem::size_of;
use device::shade::{UniformValue, ValueI32, ValueF32Vec, ProgramMeta};

pub type VarUniform = u16;
pub type VarBlock = u8;
pub type VarTexture = u8;

pub trait ParameterSink {
    fn find_uniform(&mut self, name: &str) -> Option<VarUniform>;
    fn find_block  (&mut self, name: &str) -> Option<VarBlock>;
    fn find_texture(&mut self, name: &str) -> Option<VarTexture>;
}

type MaskUniform = u64;
type MaskBlock   = u8;
type MaskTexture = u16;

pub struct MetaSink<'a> {
    prog: &'a ProgramMeta,
    mask_uni: MaskUniform,
    mask_block: MaskBlock,
    mask_tex: MaskTexture,
}

impl<'a> MetaSink<'a> {
    pub fn new(meta: &'a ProgramMeta) -> MetaSink<'a> {
        assert_eq!(0, meta.uniforms.len()>>(8*size_of::<MaskUniform>()));
        assert_eq!(0, meta.blocks  .len()>>(8*size_of::<MaskBlock  >()));
        assert_eq!(0, meta.textures.len()>>(8*size_of::<MaskTexture>()));
        MetaSink {
            prog: meta,
            mask_uni: ((1u<<meta.uniforms.len())-1u) as MaskUniform,
            mask_block: ((1u<<meta.blocks.len())-1u) as MaskBlock,
            mask_tex: ((1u<<meta.textures.len())-1u) as MaskTexture,
        }
    }

    pub fn complete(self) -> Result<(), ParameterSideError<'a>> {
        match self.prog.uniforms.iter().enumerate().find(|&(i, _)| self.mask_uni & ((1u<<i) as MaskUniform) != 0) {
            Some((_, u)) => return Err(MissingUniform(u.name.as_slice())),
            None => ()
        }
        match self.prog.blocks.iter().enumerate().find(|&(i, _)| self.mask_block & ((1u<<i) as MaskBlock) != 0) {
            Some((_, b)) => return Err(MissingBlock(b.name.as_slice())),
            None => ()
        }
        match self.prog.textures.iter().enumerate().find(|&(i, _)| self.mask_tex & ((1u<<i) as MaskTexture) != 0) {
            Some((_, t)) => return Err(MissingTexture(t.name.as_slice())),
            None => ()
        }
        Ok(())
    }
}

impl<'a> ParameterSink for MetaSink<'a>{
    fn find_uniform(&mut self, name: &str) -> Option<VarUniform> {
        self.prog.uniforms.iter().position(|u| u.name.as_slice() == name).map(|i| {
            self.mask_uni &= !(1u<<i) as MaskUniform;
            i as VarUniform
        })
    }
    fn find_block(&mut self, name: &str) -> Option<VarBlock> {
        self.prog.blocks.iter().position(|u| u.name.as_slice() == name).map(|i| {
            self.mask_block &= !(1u<<i) as MaskBlock;
            i as VarBlock
        })
    }
    fn find_texture(&mut self, name: &str) -> Option<VarTexture> {
        self.prog.textures.iter().position(|u| u.name.as_slice() == name).map(|i| {
            self.mask_tex &= !(1u<<i) as MaskTexture;
            i as VarTexture
        })
    }
}

//impl ParameterSink for ProgramMeta

pub trait Uploader {
    fn set_uniform(&mut self, VarUniform, UniformValue);
    fn set_block  (&mut self, VarBlock,   super::BufferHandle);
    fn set_texture(&mut self, VarTexture, super::TextureHandle);
}

pub trait ToUniform {
    fn to_uniform(&self) -> UniformValue;
}

impl ToUniform for i32 {
    fn to_uniform(&self) -> UniformValue {
        ValueI32(*self)
    }
}

impl ToUniform for [f32, ..4] {
    fn to_uniform(&self) -> UniformValue {
        ValueF32Vec(*self)
    }
}

#[deriving(Clone, Show)]
pub enum ParameterSideError<'a> {
    SideInternalError,
    MissingUniform(&'a str),
    MissingBlock(&'a str),
    MissingTexture(&'a str),
}

#[deriving(Clone, Show)]
pub enum ParameterLinkError<'a> {
    ErrorBadProgram,
    ErrorProgramInfo(ParameterSideError<'a>),
    ErrorShaderParam(ParameterSideError<'a>),
}

pub trait ShaderParam<L> {
    /// Creates a new link, self is passed as a workaround for Rust to not be lost in generics
    fn create_link<S: ParameterSink>(&self, &mut S) -> Result<L, ParameterSideError<'static>>;
    /// Send the parameters to the device using the Uploader implementation
    fn upload<U: Uploader>(&self, &L, &mut U);
}

pub struct ShaderBundle<L, T> {
    /// Shader program
    program: super::ProgramHandle,
    /// Global data in a user-provided struct
    pub data: T,
    /// Hidden link that provides parameter indices for user data
    link: L,
}

/// Exposing the constructor for internal use
pub trait BundleInternal<L, T> {
    fn new(Option<&Self>, super::ProgramHandle, T, L) -> ShaderBundle<L, T>;
    fn get_program(&self) -> super::ProgramHandle;
    fn bind<U: Uploader>(&self, &mut U);
}

impl<L, T: ShaderParam<L>> BundleInternal<L, T> for ShaderBundle<L, T> {
    fn new(_: Option<&ShaderBundle<L, T>>, handle: super::ProgramHandle, data: T, link: L) -> ShaderBundle<L, T> {
        ShaderBundle {
            program: handle,
            data: data,
            link: link,
        }
    }

    fn get_program(&self) -> super::ProgramHandle {
        self.program
    }

    fn bind<U: Uploader>(&self, up: &mut U) {
        self.data.upload(&self.link, up);
    }
}
