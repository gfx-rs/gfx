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
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::ops::Deref;
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
    /// Error context is full
    ContextFull,
}

/// Return type for `Batch::get_data()``
pub type BatchData<'a, R: Resources> = (&'a mesh::Mesh<R>, mesh::AttributeIter,
                                        &'a mesh::Slice<R>, &'a DrawState);

/// Abstract batch trait
#[allow(missing_docs)]
pub trait Batch {
    type Resources: Resources;
    /// Possible errors occurring at batch access
    type Error: fmt::Debug;
    /// Obtain information about the mesh, program, and state
    fn get_data(&self) -> Result<BatchData<Self::Resources>, Self::Error>;
    /// Fill shader parameter values
    fn fill_params(&self, &mut ParamStorage<Self::Resources>)
                   -> Result<&ProgramHandle<Self::Resources>, Self::Error>;
}

/// A batch that is constructed on the fly when rendering.
/// Meant to be a struct, blocked by #614
pub type ImplicitBatch<'a, T: ShaderParam> = (
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
             data: &'a T) -> ImplicitBatch<'a, T> {
    (mesh, slice, program, data, draw_state)
}

impl<'a, T: ShaderParam> Batch for ImplicitBatch<'a, T> {
    type Resources = T::Resources;
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

/// Owned batch - self-contained, but has heap-allocated data
pub struct OwnedBatch<T: ShaderParam> {
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

impl<T: ShaderParam> OwnedBatch<T> {
    /// Create a new owned batch
    pub fn new(mesh: mesh::Mesh<T::Resources>, program: ProgramHandle<T::Resources>, param: T)
           -> Result<OwnedBatch<T>, Error> {
        let slice = mesh.to_slice(PrimitiveType::TriangleList);
        let mesh_link = match mesh::Link::new(&mesh, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(Error::Mesh(e)),
        };
        let param_link = match ShaderParam::create_link(None::<&T>, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(Error::Parameters(e)),
        };
        Ok(OwnedBatch {
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

impl<T: ShaderParam> Batch for OwnedBatch<T> {
    type Resources = T::Resources;
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

type Index = u16;

//#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Id<T>(Index, PhantomData<T>);

impl<T> Copy for Id<T> {}

impl<T> Id<T> {
    fn unwrap(&self) -> Index {
        let Id(i, _) = *self;
        i
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Id(i, _) = *self;
        write!(f, "Id({})", i)
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Id<T>) -> bool {
        self.unwrap() == other.unwrap()
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Id<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Id<T>) -> Ordering {
        self.unwrap().cmp(&other.unwrap())
    }
}

struct Array<T> {
    data: Vec<T>,
    //generation: u16,
}

/// Error accessing outside of the array
#[derive(Debug)]
pub struct OutOfBounds(pub usize);

impl<T> Array<T> {
    fn new() -> Array<T> {
        Array {
            data: Vec::new(),
            //generation: 0,
        }
    }

    fn get(&self, id: Id<T>) -> Result<&T, OutOfBounds> {
        let Id(i, _) = id;
        if (i as usize) < self.data.len() {
            Ok(&self.data[i as usize])
        }else {
            Err(OutOfBounds(i as usize))
        }
    }
}

impl<T: Clone + PartialEq> Array<T> {
    fn find_or_insert(&mut self, value: &T) -> Option<Id<T>> {
        use std::num::from_u32;
        match self.data.iter().position(|v| v == value) {
            Some(i) => from_u32(i as u32).map(|id| Id(id, PhantomData)),
            None => {
                from_u32(self.data.len() as u32).map(|id| {
                    self.data.push(value.clone());
                    Id(id, PhantomData)
                })
            },
        }
    }
}


/// Referenced core - a minimal sealed batch that depends on `Context`.
/// It has references to the resources (mesh, program, state), that are held
/// by the context that created the batch, so these have to be used together.
pub struct CoreBatch<T: ShaderParam> {
    mesh_id: Id<mesh::Mesh<T::Resources>>,
    mesh_link: mesh::Link,
    program_id: Id<ProgramHandle<T::Resources>>,
    param_link: T::Link,
    state_id: Id<DrawState>,
}

impl<T: ShaderParam> Copy for CoreBatch<T> where T::Link: Copy {}

impl<T: ShaderParam> fmt::Debug for CoreBatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CoreBatch(mesh: {:?}, program: {:?}, state: {:?})",
            self.mesh_id, self.program_id, self.state_id)
    }
}

impl<T: ShaderParam> PartialEq for CoreBatch<T> {
    fn eq(&self, other: &CoreBatch<T>) -> bool {
        self.program_id == other.program_id &&
        self.state_id == other.state_id &&
        self.mesh_id == other.mesh_id
    }
}

impl<T: ShaderParam> Eq for CoreBatch<T> {}

impl<T: ShaderParam> PartialOrd for CoreBatch<T> {
    fn partial_cmp(&self, other: &CoreBatch<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: ShaderParam> Ord for CoreBatch<T> {
    fn cmp(&self, other: &CoreBatch<T>) -> Ordering {
        (&self.program_id, &self.state_id, &self.mesh_id).cmp(
        &(&other.program_id, &other.state_id, &other.mesh_id))
    }
}

impl<T: ShaderParam> CoreBatch<T> {
    /// Compare meshes by Id
    pub fn cmp_mesh(&self, other: &CoreBatch<T>) -> Ordering {
        self.mesh_id.cmp(&other.mesh_id)
    }
    /// Compare programs by Id
    pub fn cmp_program(&self, other: &CoreBatch<T>) -> Ordering {
        self.program_id.cmp(&other.program_id)
    }
    /// Compare draw states by Id
    pub fn cmp_state(&self, other: &CoreBatch<T>) -> Ordering {
        self.state_id.cmp(&other.state_id)
    }
}

/// A `CoreBatch` completed by a slice, shader parameters, and a context
/// Implements `Batch` thus can be drawn.
/// It is meant to be a struct, but we have lots of lifetime issues
/// with associated resources, binding which looks nasty (#614)
pub type CoreBatchFull<'a, T: ShaderParam> = (
    &'a CoreBatch<T>,
    &'a mesh::Slice<T::Resources>,
    &'a T,
    &'a Context<T::Resources>
);


/// An expanded version of the `CoreBatch`, carrying the parameters and
/// the mesh slice with it, publicly mutable.
pub struct RefBatch<T: ShaderParam> {
    /// Core of the batch
    pub core: CoreBatch<T>,
    /// Mesh slice
    pub slice: mesh::Slice<T::Resources>,
    /// Shader parameter values
    pub params: T,
}

impl<T: ShaderParam> Deref for RefBatch<T> {
    type Target = CoreBatch<T>;

    fn deref(&self) -> &CoreBatch<T> {
        &self.core
    }
}

impl<T: ShaderParam + Clone> Clone for RefBatch<T> where T::Link: Copy {
    fn clone(&self) -> RefBatch<T> {
        RefBatch {
            core: self.core,
            slice: self.slice.clone(),
            params: self.params.clone(),
        }
    }
}

impl<T: ShaderParam + fmt::Debug> fmt::Debug for RefBatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RefBatch(core: {:?}, slice: {:?}, params: {:?}",
            self.core, self.slice, self.params)
    }
}


/// Factory of ref batches, required to always be used with them.
pub struct Context<R: Resources> {
    meshes: Array<mesh::Mesh<R>>,
    programs: Array<ProgramHandle<R>>,
    states: Array<DrawState>,
}

/// A `RefBatch` completed by the a batch context
/// Implements `Batch` thus can be drawn.
/// It is meant to be a struct, but we have lots of lifetime issues
/// with associated resources, binding which looks nasty (#614)
pub type RefBatchFull<'a, T: ShaderParam> = (
    &'a RefBatch<T>,
    &'a Context<T::Resources>
);

impl<R: Resources> Context<R> {
    /// Create a new empty `Context`
    pub fn new() -> Context<R> {
        Context {
            meshes: Array::new(),
            programs: Array::new(),
            states: Array::new(),
        }
    }
}

impl<R: Resources> Context<R> {
    fn make<T: ShaderParam<Resources = R>>(&mut self,
            program: &ProgramHandle<R>,
            params: Option<&T>,
            mesh: &mesh::Mesh<R>,
            state: &DrawState)
            -> Result<CoreBatch<T>, Error> {
        let mesh_link = match mesh::Link::new(mesh, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(Error::Mesh(e)),
        };
        let link = match ShaderParam::create_link(params, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(Error::Parameters(e))
        };
        let mesh_id = match self.meshes.find_or_insert(mesh) {
            Some(id) => id,
            None => return Err(Error::ContextFull),
        };
        let program_id = match self.programs.find_or_insert(program) {
            Some(id) => id,
            None => return Err(Error::ContextFull),
        };
        let state_id = match self.states.find_or_insert(state) {
            Some(id) => id,
            None => return Err(Error::ContextFull),
        };

        Ok(CoreBatch {
            mesh_id: mesh_id,
            mesh_link: mesh_link,
            program_id: program_id,
            param_link: link,
            state_id: state_id,
        })
    }

    /// Produce a new `CoreBatch`
    pub fn make_core<T: ShaderParam<Resources = R>>(&mut self,
                     program: &ProgramHandle<R>,
                     mesh: &mesh::Mesh<R>,
                     state: &DrawState)
                     -> Result<CoreBatch<T>, Error> {
        self.make(program, None, mesh, state)
    }

    /// Produce a new `RefBatch`
    pub fn make_batch<T: ShaderParam<Resources = R>>(&mut self,
                      program: &ProgramHandle<R>,
                      params: T,
                      mesh: &mesh::Mesh<R>,
                      slice: mesh::Slice<R>,
                      state: &DrawState)
                      -> Result<RefBatch<T>, Error> {
        self.make(program, Some(&params), mesh, state)
            .map(|core| RefBatch {
            core: core,
            slice: slice,
            params: params,
        })
    }

    /// Complete a CoreBatch temporarily by turning it into CoreBatchFull
    pub fn bind<'a, T: ShaderParam<Resources = R> + 'a>(&'a self,
                 core: &'a CoreBatch<T>, slice: &'a mesh::Slice<R>,
                 params: &'a T) -> CoreBatchFull<'a, T> {
        (core, slice, params, self)
    }

    /// Get data from a batch in the format required for `Batch`
    pub fn get_data<'a, T: ShaderParam<Resources = R> + 'a>(&'a self,
                    core: &CoreBatch<T>, slice: &'a mesh::Slice<R>)
                    -> Result<BatchData<'a, T::Resources>, OutOfBounds> {
        Ok((try!(self.meshes.get(core.mesh_id)),
            core.mesh_link.to_iter(),
            slice,
            try!(self.states.get(core.state_id))
        ))
    }
}

impl<'a, T: ShaderParam + 'a> Batch for CoreBatchFull<'a, T> {
    type Resources = T::Resources;
    type Error = OutOfBounds;

    fn get_data(&self) -> Result<BatchData<T::Resources>, OutOfBounds> {
        let (b, slice, _, ctx) = *self;
        ctx.get_data(b, slice)
    }

    fn fill_params(&self, values: &mut ParamStorage<T::Resources>)
                   -> Result<&ProgramHandle<T::Resources>, OutOfBounds> {
        let (b, _, data, ctx) = *self;
        match ctx.programs.get(b.program_id) {
            Ok(program) => {
                values.reserve(program.get_info());
                data.fill_params(&b.param_link, values);
                Ok(program)
            },
            e => e,
        }
    }
}

impl<'a, T: ShaderParam + 'a> Batch for RefBatchFull<'a, T> {
    type Resources = T::Resources;
    type Error = OutOfBounds;

    fn get_data(&self) -> Result<BatchData<T::Resources>, OutOfBounds> {
        let (b, ctx) = *self;
        ctx.get_data(&b.core, &b.slice)
    }

    fn fill_params(&self, values: &mut ParamStorage<T::Resources>)
                   -> Result<&ProgramHandle<T::Resources>, OutOfBounds> {
        let (b, ctx) = *self;
        match ctx.programs.get(b.core.program_id) {
            Ok(program) => {
                values.reserve(program.get_info());
                b.params.fill_params(&b.core.param_link, values);
                Ok(program)
            },
            e => e,
        }
    }
}
