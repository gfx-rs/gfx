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

//! Batches are structures containing all the data required for the draw call,
//! except for the target frame. Here we define the `Batch` trait as well as
//! `RefBatch` and `OwnedBatch` implementations.

use std::fmt;
use draw_state::DrawState;

use device::{Resources, PrimitiveType};
use device::handle::Program as ProgramHandle;
use render::mesh;
use render::mesh::ToSlice;
use shade::{ParameterError, ShaderParam};
use super::ParamStorage;

/// An error occurring at batch creation
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Error connecting mesh attributes
    Mesh(mesh::Error),
    /// Error connecting shader parameters
    Parameters(ParameterError),
}

/// Return type for `Batch::get_data()``
pub type BatchData<'a, R: Resources> = (&'a mesh::Mesh<R>, mesh::AttributeIter,
                                        &'a mesh::Slice<R>, &'a DrawState);

/// Abstract batch trait
pub trait Batch<R: Resources> {
    /// Possible errors occurring at batch access
    type Error: fmt::Debug;
    /// Obtain information about the mesh, program, and state
    fn get_data(&self) -> Result<BatchData<R>, Self::Error>;
    /// Fill shader parameter values
    fn fill_params(&self, &mut ParamStorage<R>)
                   -> Result<&ProgramHandle<R>, Self::Error>;
}

/// A batch that is constructed on the fly when rendering.
/// Meant to be a struct, blocked by #614
pub type Implicit<'a, T: ShaderParam> = (
    &'a mesh::Mesh<T::Resources>,
    mesh::Slice<T::Resources>,
    &'a ProgramHandle<T::Resources>,
    &'a T,
    &'a DrawState
);

/// Create an implicit batch
pub fn bind<'a, T: ShaderParam>(draw_state: &'a DrawState,
             mesh: &'a mesh::Mesh<T::Resources>,
             slice: mesh::Slice<T::Resources>,
             program: &'a ProgramHandle<T::Resources>,
             data: &'a T) -> Implicit<'a, T> {
    (mesh, slice, program, data, draw_state)
}

impl<'a, T: ShaderParam> Batch<T::Resources> for Implicit<'a, T> {
    type Error = Error;

    fn get_data(&self) -> Result<BatchData<T::Resources>, Error> {
        let (mesh, ref slice, program, _, state) = *self;
        match mesh::Link::new(mesh, program.get_info()) {
            Ok(link) => Ok((mesh, link.to_iter(), &slice, state)),
            Err(e) => Err(Error::Mesh(e)),
        }
    }

    fn fill_params(&self, values: &mut ParamStorage<T::Resources>)
                   -> Result<&ProgramHandle<T::Resources>, Error> {
        let (_, _, program, params, _) = *self;
        match ShaderParam::create_link(None::<&T>, program.get_info()) {
            Ok(link) => {
                values.reserve(program.get_info());
                params.fill_params(&link, values);
                Ok(program)
            },
            Err(e) => return Err(Error::Parameters(e)),
        }
    }
}

/// Full batch - contains everything needed for rendering.
#[derive(Clone)]
pub struct Full<T: ShaderParam> {
    mesh: mesh::Mesh<T::Resources>,
    mesh_link: mesh::Link,
    /// Mesh slice
    pub slice: mesh::Slice<T::Resources>,
    /// Parameter data.
    pub param: T,
    program: ProgramHandle<T::Resources>,
    param_link: T::Link,
    /// Draw state
    pub state: DrawState,
}

impl<T: ShaderParam> Full<T> {
    /// Create a new full batch
    pub fn new(mesh: mesh::Mesh<T::Resources>, program: ProgramHandle<T::Resources>, param: T)
           -> Result<Full<T>, Error> {
        let slice = mesh.to_slice(PrimitiveType::TriangleList);
        let mesh_link = match mesh::Link::new(&mesh, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(Error::Mesh(e)),
        };
        let param_link = match ShaderParam::create_link(Some(&param), program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(Error::Parameters(e)),
        };
        Ok(Full {
            mesh: mesh,
            mesh_link: mesh_link,
            slice: slice,
            program: program,
            param: param,
            param_link: param_link,
            state: DrawState::new(),
        })
    }
}

impl<T: ShaderParam> Batch<T::Resources> for Full<T> {
    type Error = ();

    fn get_data(&self) -> Result<BatchData<T::Resources>, ()> {
        Ok((&self.mesh, self.mesh_link.to_iter(), &self.slice, &self.state))
    }

    fn fill_params(&self, values: &mut ParamStorage<T::Resources>)
                   -> Result<&ProgramHandle<T::Resources>, ()> {
        values.reserve(self.program.get_info());
        self.param.fill_params(&self.param_link, values);
        Ok(&self.program)
    }
}


/// Core batch - a minimal sealed batch.
#[derive(Clone)]
pub struct Core<T: ShaderParam> {
    mesh: mesh::Mesh<T::Resources>,
    mesh_link: mesh::Link,
    program: ProgramHandle<T::Resources>,
    param_link: T::Link,
}

/// A `Core` completed by a mesh slice, shader parameters, and a state.
/// Implements `Batch` thus can be drawn.
/// It is meant to be a struct, but we have lots of lifetime issues
/// with associated resources, binding which looks nasty (#614)
pub type Complete<'a, T: ShaderParam> = (
    &'a Core<T>,
    &'a mesh::Slice<T::Resources>,
    &'a T,
    &'a DrawState
);

impl<T: ShaderParam> Core<T> {
    /// Create a new core batch.
    pub fn new(mesh: mesh::Mesh<T::Resources>, program: ProgramHandle<T::Resources>)
           -> Result<Core<T>, Error> {
        let mesh_link = match mesh::Link::new(&mesh, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(Error::Mesh(e)),
        };
        let param_link = match ShaderParam::create_link(None::<&T>, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(Error::Parameters(e)),
        };
        Ok(Core {
            mesh: mesh,
            mesh_link: mesh_link,
            program: program,
            param_link: param_link,
        })
    }

    /// Add missing components to complete the batch for rendering.
    pub fn with<'a>(&'a self, slice: &'a mesh::Slice<T::Resources>,
                params: &'a T, state: &'a DrawState)
                -> Complete<'a, T> {
        (self, slice, params, state)
    }
}

impl<'a, T: ShaderParam + 'a> Batch<T::Resources> for Complete<'a, T> {
    type Error = ();

    fn get_data(&self) -> Result<BatchData<T::Resources>, ()> {
        let (b, slice, _, state) = *self;
        Ok((&b.mesh, b.mesh_link.to_iter(), slice, state))
    }

    fn fill_params(&self, values: &mut ParamStorage<T::Resources>)
                   -> Result<&ProgramHandle<T::Resources>, ()> {
        let (b, _, data, _) = *self;
        values.reserve(b.program.get_info());
        data.fill_params(&b.param_link, values);
        Ok(&b.program)
    }
}
