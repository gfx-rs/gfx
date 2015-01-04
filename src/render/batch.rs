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
use std::num::from_uint;
use std::cmp::Ordering;
use device::{PrimitiveType, ProgramHandle};
use device::shade::ProgramInfo;
use mesh;
use mesh::ToSlice;
use shade::{ParameterError, ShaderParam};
use state::DrawState;

/// An error with a defined Mesh.
#[derive(Clone, Show)]
pub enum MeshError {
    /// A required attribute was missing.
    AttributeMissing(String),
    /// An attribute's type from the vertex format differed from the type used in the shader.
    AttributeType,
    /// Internal error due to mesh link limitations
    MeshLink(mesh::LinkError),
}

/// An error occurring at batch creation
#[derive(Clone, Show)]
pub enum BatchError {
    /// Error connecting mesh attributes
    Mesh(MeshError),
    /// Error connecting shader parameters
    Parameters(ParameterError),
    /// Error context is full
    ContextFull,
}

/// Match mesh attributes against shader inputs, produce a mesh link.
/// Exposed to public to allow external `Batch` implementations to use it.
pub fn link_mesh(mesh: &mesh::Mesh, pinfo: &ProgramInfo) -> Result<mesh::Link, MeshError> {
    let mut indices = Vec::new();
    for sat in pinfo.attributes.iter() {
        match mesh.attributes.iter().enumerate()
                  .find(|&(_, a)| a.name.as_slice() == sat.name.as_slice()) {
            Some((attrib_id, vat)) => match vat.format.elem_type.is_compatible(sat.base_type) {
                Ok(_) => indices.push(attrib_id),
                Err(_) => return Err(MeshError::AttributeType),
            },
            None => return Err(MeshError::AttributeMissing(sat.name.clone())),
        }
    }
    mesh::Link::from_iter(indices.into_iter())
        .map_err(|e| MeshError::MeshLink(e))
}

/// Abstract batch trait
pub trait Batch {
    /// Obtain information about the mesh, program, and state
    fn get_data(&self) -> (&mesh::Mesh, &mesh::Link, &mesh::Slice, &ProgramHandle, &DrawState);
    /// Fill shader parameter values
    fn fill_params(&self, ::shade::ParamValues);
}

/// Owned batch - self-contained, but has heap-allocated data
pub struct OwnedBatch<L, T> {
    mesh: mesh::Mesh,
    mesh_link: mesh::Link,
    /// Mesh slice
    pub slice: mesh::Slice,
    /// Parameter data.
    pub param: T,
    program: ProgramHandle,
    param_link: L,
    /// Draw state
    pub state: DrawState,
}

impl<L, T: ShaderParam<L>> OwnedBatch<L, T> {
    /// Create a new owned batch
    pub fn new(mesh: mesh::Mesh, program: ProgramHandle, param: T)
           -> Result<OwnedBatch<L, T>, BatchError> {
        let slice = mesh.to_slice(PrimitiveType::TriangleList);
        let mesh_link = match link_mesh(&mesh, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(BatchError::Mesh(e)),
        };
        let param_link = match ShaderParam::create_link(None::<&T>, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(BatchError::Parameters(e)),
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

impl<L, T: ShaderParam<L>> Batch for OwnedBatch<L, T> {
    fn get_data(&self) -> (&mesh::Mesh, &mesh::Link, &mesh::Slice, &ProgramHandle, &DrawState) {
        (&self.mesh, &self.mesh_link, &self.slice, &self.program, &self.state)
    }

    fn fill_params(&self, values: ::shade::ParamValues) {
        self.param.fill_params(&self.param_link, values);
    }
}

type Index = u16;

//#[derive(PartialEq, Eq, PartialOrd, Ord, Show)]
struct Id<T>(Index);

impl<T> Copy for Id<T> {}

impl<T> Id<T> {
    fn unwrap(&self) -> Index {
        let Id(i) = *self;
        i
    }
}

impl<T> fmt::Show for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Id(i) = *self;
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

impl<T> Array<T> {
    fn new() -> Array<T> {
        Array {
            data: Vec::new(),
            //generation: 0,
        }
    }

    fn get(&self, id: Id<T>) -> &T {
        let Id(i) = id;
        &self.data[i as uint]
    }
}

impl<T: Clone + PartialEq> Array<T> {
    fn find_or_insert(&mut self, value: &T) -> Option<Id<T>> {
        match self.data.iter().position(|v| v == value) {
            Some(i) => from_uint::<Index>(i).map(|id| Id(id)),
            None => {
                from_uint::<Index>(self.data.len()).map(|id| {
                    self.data.push(value.clone());
                    Id(id)
                })
            },
        }
    }
}


/// Ref batch - copyable and smaller, but depends on the `Context`.
/// It has references to the resources (mesh, program, state), that are held
/// by the context that created the batch, so these have to be used together.
pub struct RefBatch<L, T> {
    mesh_id: Id<mesh::Mesh>,
    mesh_link: mesh::Link,
    /// Mesh slice
    pub slice: mesh::Slice,
    program_id: Id<ProgramHandle>,
    param_link: L,
    state_id: Id<DrawState>,
}

impl<L, T> fmt::Show for RefBatch<L, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RefBatch(mesh: {}, slice: {}, program: {}, state: {})",
            self.mesh_id, self.slice, self.program_id, self.state_id)
    }
}

impl<L, T> PartialEq for RefBatch<L, T> {
    fn eq(&self, other: &RefBatch<L, T>) -> bool {
        self.program_id == other.program_id &&
        self.state_id == other.state_id &&
        self.mesh_id == other.mesh_id
    }
}

impl<L, T> Eq for RefBatch<L, T> {}

impl<L, T> PartialOrd for RefBatch<L, T> {
    fn partial_cmp(&self, other: &RefBatch<L, T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<L, T> Ord for RefBatch<L, T> {
    fn cmp(&self, other: &RefBatch<L, T>) -> Ordering {
        (&self.program_id, &self.state_id, &self.mesh_id).cmp(
        &(&other.program_id, &other.state_id, &other.mesh_id))
    }
}

/// Factory of ref batches, required to always be used with them.
pub struct Context {
    meshes: Array<mesh::Mesh>,
    programs: Array<ProgramHandle>,
    states: Array<DrawState>,
}

impl Context {
    /// Create a new empty `Context`
    pub fn new() -> Context {
        Context {
            meshes: Array::new(),
            programs: Array::new(),
            states: Array::new(),
        }
    }
}

impl Context {
    /// Produce a new ref batch
    pub fn make_batch<L, T: ShaderParam<L>>(&mut self,
                      program: &ProgramHandle,
                      mesh: &mesh::Mesh,
                      slice: mesh::Slice,
                      state: &DrawState)
                      -> Result<RefBatch<L, T>, BatchError> {
        let mesh_link = match link_mesh(mesh, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(BatchError::Mesh(e)),
        };
        let link = match ShaderParam::create_link(None::<&T>, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(BatchError::Parameters(e))
        };
        let mesh_id = match self.meshes.find_or_insert(mesh) {
            Some(id) => id,
            None => return Err(BatchError::ContextFull),
        };
        let program_id = match self.programs.find_or_insert(program) {
            Some(id) => id,
            None => return Err(BatchError::ContextFull),
        };
        let state_id = match self.states.find_or_insert(state) {
            Some(id) => id,
            None => return Err(BatchError::ContextFull),
        };

        Ok(RefBatch {
            mesh_id: mesh_id,
            mesh_link: mesh_link,
            slice: slice,
            program_id: program_id,
            param_link: link,
            state_id: state_id,
        })
    }
}

impl<'a, L, T: ShaderParam<L>> Batch for (&'a RefBatch<L, T>, &'a T, &'a Context) {
    fn get_data(&self) -> (&mesh::Mesh, &mesh::Link, &mesh::Slice, &ProgramHandle, &DrawState) {
        let (b, _, ctx) = *self;
        (ctx.meshes.get(b.mesh_id),
         &b.mesh_link,
         &b.slice,
         ctx.programs.get(b.program_id),
         ctx.states.get(b.state_id))
    }

    fn fill_params(&self, values: ::shade::ParamValues) {
        let (b, data, _) = *self;
        data.fill_params(&b.param_link, values);
    }
}
