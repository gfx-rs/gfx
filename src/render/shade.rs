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
use dev = device::shade;

pub type VarUniform = u16;
pub type VarBlock = u8;
pub type VarTexture = u8;

/// Something that has information about program parameters,
/// used to fill up a hidden Link structure for the `ShaderParam` implemntor
pub trait ParameterSink {
    fn find_uniform(&mut self, name: &str) -> Option<VarUniform>;
    fn find_block  (&mut self, name: &str) -> Option<VarBlock>;
    fn find_texture(&mut self, name: &str) -> Option<VarTexture>;
}

type MaskUniform = u64;
type MaskBlock   = u8;
type MaskTexture = u16;

/// A `ProgramMeta` wrapper that keeps track of the queried variable indices.
/// It returns an error if a shader parameter is not being asked for.
pub struct MetaSink<'a> {
    prog: &'a dev::ProgramMeta,
    mask_uni: MaskUniform,
    mask_block: MaskBlock,
    mask_tex: MaskTexture,
}

impl<'a> MetaSink<'a> {
    /// Creates a new wrapper
    pub fn new(meta: &'a dev::ProgramMeta) -> MetaSink<'a> {
        debug_assert_eq!(0, meta.uniforms.len() >> (8 * size_of::<MaskUniform>()));
        debug_assert_eq!(0, meta.blocks  .len() >> (8 * size_of::<MaskBlock  >()));
        debug_assert_eq!(0, meta.textures.len() >> (8 * size_of::<MaskTexture>()));
        MetaSink {
            prog: meta,
            mask_uni: ((1u << meta.uniforms.len()) - 1u) as MaskUniform,
            mask_block: ((1u << meta.blocks.len()) - 1u) as MaskBlock,
            mask_tex: ((1u << meta.textures.len()) - 1u) as MaskTexture,
        }
    }

    /// Finalizes the wrapper, checking that all the parameters are used
    pub fn complete(self) -> Result<(), ParameterError<'a>> {
        match self.prog.uniforms.iter().enumerate().find(|&(i, _)| self.mask_uni & ((1u << i) as MaskUniform) != 0) {
            Some((_, u)) => return Err(ErrorUniform(u.name.as_slice())),
            None => ()
        }
        match self.prog.blocks.iter().enumerate().find(|&(i, _)| self.mask_block & ((1u << i) as MaskBlock) != 0) {
            Some((_, b)) => return Err(ErrorBlock(b.name.as_slice())),
            None => ()
        }
        match self.prog.textures.iter().enumerate().find(|&(i, _)| self.mask_tex & ((1u << i) as MaskTexture) != 0) {
            Some((_, t)) => return Err(ErrorTexture(t.name.as_slice())),
            None => ()
        }
        Ok(())
    }
}

impl<'a> ParameterSink for MetaSink<'a>{
    fn find_uniform(&mut self, name: &str) -> Option<VarUniform> {
        self.prog.uniforms.iter().position(|u| u.name.as_slice() == name).map(|i| {
            self.mask_uni &= !(1u << i) as MaskUniform;
            i as VarUniform
        })
    }
    fn find_block(&mut self, name: &str) -> Option<VarBlock> {
        self.prog.blocks.iter().position(|u| u.name.as_slice() == name).map(|i| {
            self.mask_block &= !(1u << i) as MaskBlock;
            i as VarBlock
        })
    }
    fn find_texture(&mut self, name: &str) -> Option<VarTexture> {
        self.prog.textures.iter().position(|u| u.name.as_slice() == name).map(|i| {
            self.mask_tex &= !(1u << i) as MaskTexture;
            i as VarTexture
        })
    }
}


/// Helper trait to transform base types into their corresponding uniforms
pub trait ToUniform {
    fn to_uniform(&self) -> dev::UniformValue;
}

impl ToUniform for i32 {
    fn to_uniform(&self) -> dev::UniformValue {
        dev::ValueI32(*self)
    }
}

impl ToUniform for f32 {
    fn to_uniform(&self) -> dev::UniformValue {
        dev::ValueF32(*self)
    }
}

impl ToUniform for [i32, ..4] {
    fn to_uniform(&self) -> dev::UniformValue {
        dev::ValueI32Vec(*self)
    }
}

impl ToUniform for [f32, ..4] {
    fn to_uniform(&self) -> dev::UniformValue {
        dev::ValueF32Vec(*self)
    }
}

impl ToUniform for [[f32, ..4], ..4] {
    fn to_uniform(&self) -> dev::UniformValue {
        dev::ValueF32Matrix(*self)
    }
}

/// A closure provided for the `ShaderParam` implementor for uploading
pub type FnUniform<'a> = |VarUniform, dev::UniformValue|: 'a;
pub type FnBlock  <'a> = |VarBlock, super::BufferHandle|: 'a;
pub type FnTexture<'a> = |VarTexture, super::TextureHandle|: 'a;


/// An error type on either the parameter storage or the program side
#[deriving(Clone, Show)]
pub enum ParameterError<'a> {
    ErrorInternal,
    ErrorUniform(&'a str),
    ErrorBlock(&'a str),
    ErrorTexture(&'a str),
}

/// An error type for the link cretion
#[deriving(Clone, Show)]
pub enum ParameterLinkError<'a> {
    /// Program is not valid
    ErrorBadProgram,
    /// A given parameter is not used by the program
    ErrorUnusedParameter(ParameterError<'a>),
    /// A program parameter that is not provided
    ErrorMissingParameter(ParameterError<'a>),
}

/// Main trait that is generated for a user data structure with the `shader_param` attribute
pub trait ShaderParam<L> {
    /// Creates a new link, self is passed as a workaround for Rust to not be lost in generics
    fn create_link<S: ParameterSink>(&self, &mut S) -> Result<L, ParameterError<'static>>;
    /// Send the parameters to the device using the Uploader closures
    fn upload<'a>(&self, &L, FnUniform<'a>, FnBlock<'a>, FnTexture<'a>);
}

/// A bundle that encapsulates a program, its data, and a hidden link between them
#[deriving(Clone)]
pub struct ShaderBundle<L, T> {
    /// Shader program
    program: super::ProgramHandle,
    /// Global data in a user-provided struct
    pub data: T,
    /// Hidden link that provides parameter indices for user data
    link: L,
}

/// Helper trait to expose some abilities for internal use by the `Renderer`
pub trait BundleInternal<L, T> {
    fn new(Option<&Self>, super::ProgramHandle, T, L) -> ShaderBundle<L, T>;
    fn get_program(&self) -> super::ProgramHandle;
    fn bind<'a>(&self, FnUniform<'a>, FnBlock<'a>, FnTexture<'a>);
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

    fn bind<'a>(&self, fu: FnUniform<'a>, fb: FnBlock<'a>, ft: FnTexture<'a>) {
        self.data.upload(&self.link, fu, fb, ft);
    }
}
