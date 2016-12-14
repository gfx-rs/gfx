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

//! Factory extension.
//!
//! This module serves as an extension to the `factory` module in the `gfx` crate. This module
//! exposes extension functions and shortcuts to aid with creating and managing graphics resources.
//! See the `FactoryExt` trait for more information.

use std::error::Error;
use std::fmt;
use core::{buffer, format, handle, mapping, texture, state};
use core::{Primitive, Resources, ShaderSet};
use core::factory::Factory;
use core::pso::{CreationError, Descriptor};
use core::memory::{self, Bind, Pod};
use slice::{Slice, IndexBuffer, IntoIndexBuffer};
use pso;
use shade::ProgramError;

/// Error creating a PipelineState
#[derive(Clone, PartialEq, Debug)]
pub enum PipelineStateError<S> {
    /// Shader program failed to link.
    Program(ProgramError),
    /// Unable to create PSO descriptor due to mismatched formats.
    DescriptorInit(pso::InitError<S>),
    /// Device failed to create the handle give the descriptor.
    DeviceCreate(CreationError),
}

impl<S: Error> fmt::Display for PipelineStateError<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PipelineStateError::Program(ref e) => write!(f, "{}: {}", self.description(), e),
            PipelineStateError::DescriptorInit(ref e) => write!(f, "{}: {}", self.description(), e),
            PipelineStateError::DeviceCreate(ref e) => write!(f, "{}: {}", self.description(), e),
        }
    }
}

impl<S: Error> Error for PipelineStateError<S> {
    fn description(&self) -> &str {
        match *self {
            PipelineStateError::Program(_) => "Shader program failed to link",
            PipelineStateError::DescriptorInit(_) =>
                "Unable to create PSO descriptor due to mismatched formats",
            PipelineStateError::DeviceCreate(_) => "Device failed to create the handle give the descriptor",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            PipelineStateError::Program(ref program_error) => Some(program_error),
            PipelineStateError::DescriptorInit(ref init_error) => Some(init_error),
            PipelineStateError::DeviceCreate(ref creation_error) => Some(creation_error),
        }
    }
}

impl<S> From<ProgramError> for PipelineStateError<S> {
    fn from(e: ProgramError) -> Self {
        PipelineStateError::Program(e)
    }
}

impl<S> From<pso::InitError<S>> for PipelineStateError<S> {
    fn from(e: pso::InitError<S>) -> Self {
        PipelineStateError::DescriptorInit(e)
    }
}

impl<S> From<CreationError> for PipelineStateError<S> {
    fn from(e: CreationError) -> Self {
        PipelineStateError::DeviceCreate(e)
    }
}

/// This trait is responsible for creating and managing graphics resources, much like the `Factory`
/// trait in the `gfx` crate. Every `Factory` automatically implements `FactoryExt`. 
pub trait FactoryExt<R: Resources>: Factory<R> {
    /// Create a vertex buffer from the supplied data. A `Slice` will have to manually be
    /// constructed.
    fn create_vertex_buffer<T>(&mut self, vertices: &[T])
                            -> handle::Buffer<R, T> where
                            T: Pod + pso::buffer::Structure<format::Format>
    {
        //debug_assert!(nv <= self.get_capabilities().max_vertex_count);
        self.create_buffer_immutable(vertices, buffer::Role::Vertex, Bind::empty()).unwrap()
    }
    
    /// Shorthand for creating a new vertex buffer from the supplied vertices, together with a
    /// `Slice` from the supplied indices.
    fn create_vertex_buffer_with_slice<B, V>(&mut self, vertices: &[V], indices: B)
                                       -> (handle::Buffer<R, V>, Slice<R>)
                                       where V: Pod + pso::buffer::Structure<format::Format>,
                                             B: IntoIndexBuffer<R>
    {
        let vertex_buffer = self.create_vertex_buffer(vertices);
        let index_buffer = indices.into_index_buffer(self);
        let buffer_length = match index_buffer {
            IndexBuffer::Auto => vertex_buffer.len(),
            IndexBuffer::Index16(ref ib) => ib.len(),
            IndexBuffer::Index32(ref ib) => ib.len(),
        };
        
        (vertex_buffer, Slice {
            start: 0,
            end: buffer_length as u32,
            base_vertex: 0,
            instances: None,
            buffer: index_buffer
        })
    }

    /// Create a constant buffer for `num` identical elements of type `T`.
    fn create_constant_buffer<T>(&mut self, num: usize) -> handle::Buffer<R, T> {
        self.create_buffer_dynamic(num, buffer::Role::Constant, Bind::empty())
            .unwrap()
    }

    /// Creates and maps a readable persistent buffer.
    fn create_buffer_persistent_readable<T>(&mut self,
                                            num: usize,
                                            role: buffer::Role,
                                            bind: Bind) -> (handle::Buffer<R, T>,
                                                            mapping::Readable<R, T>)
        where T: Copy
    {
        let buffer = self.create_buffer_persistent(num, role, bind, memory::READ)
            .unwrap();
        let mapping = self.map_buffer_readable(&buffer).unwrap();
        (buffer, mapping)
    }

    /// Creates and maps a writable persistent buffer.
    fn create_buffer_persistent_writable<T>(&mut self,
                                            num: usize,
                                            role: buffer::Role,
                                            bind: Bind) -> (handle::Buffer<R, T>,
                                                            mapping::Writable<R, T>)
        where T: Copy
    {
        let buffer = self.create_buffer_persistent(num, role, bind, memory::WRITE)
            .unwrap();
        let mapping = self.map_buffer_writable(&buffer).unwrap();
        (buffer, mapping)
    }

    /// Creates and maps an rw persistent buffer.
    fn create_buffer_persistent_rw<T>(&mut self,
                                      num: usize,
                                      role: buffer::Role,
                                      bind: Bind) -> (handle::Buffer<R, T>,
                                                      mapping::RWable<R, T>)
        where T: Copy
    {
        let buffer = self.create_buffer_persistent(num, role, bind, memory::RW)
            .unwrap();
        let mapping = self.map_buffer_rw(&buffer).unwrap();
        (buffer, mapping)
    }

    /// Creates a `ShaderSet` from the supplied vertex and pixel shader source code.
    fn create_shader_set(&mut self, vs_code: &[u8], ps_code: &[u8])
                         -> Result<ShaderSet<R>, ProgramError> {
        let vs = match self.create_shader_vertex(vs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };
        let ps = match self.create_shader_pixel(ps_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Pixel(e)),
        };
        Ok(ShaderSet::Simple(vs, ps))
    }

    /// Mainly for testing
    fn create_shader_set_tessellation(&mut self, vs_code: &[u8], hs_code: &[u8], ds_code: &[u8], ps_code: &[u8])
                         -> Result<ShaderSet<R>, ProgramError> {
        let vs = match self.create_shader_vertex(vs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Vertex(e)),
        };

        let hs = match self.create_shader_hull(hs_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Hull(e)),
        };

        let ds = match self.create_shader_domain(ds_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Domain(e)),
        };

        let ps = match self.create_shader_pixel(ps_code) {
            Ok(s) => s,
            Err(e) => return Err(ProgramError::Pixel(e)),
        };
        Ok(ShaderSet::Tessellated(vs, hs, ds, ps))
    }

    /// Creates a basic shader `Program` from the supplied vertex and pixel shader source code.
    fn link_program(&mut self, vs_code: &[u8], ps_code: &[u8])
                    -> Result<handle::Program<R>, ProgramError> {

        let set = try!(self.create_shader_set(vs_code, ps_code));
        self.create_program(&set).map_err(|e| ProgramError::Link(e))
    }

    /// Similar to `create_pipeline_from_program(..)`, but takes a `ShaderSet` as opposed to a
    /// shader `Program`.  
    fn create_pipeline_state<I: pso::PipelineInit>(&mut self, shaders: &ShaderSet<R>,
                             primitive: Primitive, rasterizer: state::Rasterizer, init: I)
                             -> Result<pso::PipelineState<R, I::Meta>, PipelineStateError<String>>
    {
        let program = try!(self.create_program(shaders).map_err(|e| ProgramError::Link(e)));
        self.create_pipeline_from_program(&program, primitive, rasterizer, init).map_err(|error| {
            use self::PipelineStateError::*;
            match error {
                Program(e) => Program(e),
                DescriptorInit(e) => DescriptorInit(e.into()),
                DeviceCreate(e) => DeviceCreate(e),
            }
        })
    }

    /// Creates a strongly typed `PipelineState` from its `Init` structure, a shader `Program`, a
    /// primitive type and a `Rasterizer`.
    fn create_pipeline_from_program<'a, I: pso::PipelineInit>(&mut self, program: &'a handle::Program<R>,
                                    primitive: Primitive, rasterizer: state::Rasterizer, init: I)
                                    -> Result<pso::PipelineState<R, I::Meta>, PipelineStateError<&'a str>>
    {
        let mut descriptor = Descriptor::new(primitive, rasterizer);
        let meta = try!(init.link_to(&mut descriptor, program.get_info()));
        let raw = try!(self.create_pipeline_state_raw(program, &descriptor));

        Ok(pso::PipelineState::new(raw, primitive, meta))
    }

    /// Creates a strongly typed `PipelineState` from its `Init` structure. Automatically creates a
    /// shader `Program` from a vertex and pixel shader source, as well as a `Rasterizer` capable
    /// of rendering triangle faces without culling.
    fn create_pipeline_simple<I: pso::PipelineInit>(&mut self, vs: &[u8], ps: &[u8], init: I)
                              -> Result<pso::PipelineState<R, I::Meta>, PipelineStateError<String>>
    {
        let set = try!(self.create_shader_set(vs, ps));
        self.create_pipeline_state(&set, Primitive::TriangleList, state::Rasterizer::new_fill(),
                                   init)
    }

    /// Create a linear sampler with clamping to border.
    fn create_sampler_linear(&mut self) -> handle::Sampler<R> {
        self.create_sampler(texture::SamplerInfo::new(
            texture::FilterMethod::Trilinear,
            texture::WrapMode::Clamp,
        ))
    }
}

impl<R: Resources, F: Factory<R>> FactoryExt<R> for F {}
