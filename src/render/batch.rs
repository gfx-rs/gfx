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
//! `LightBatch` and `HeavyBatch` implementations.

use device::ProgramHandle;
use mesh::{Mesh, Slice};
use shade::{ParameterError, ShaderParam};
use state::DrawState;

struct MeshLink; //TODO

/// An error occurring at batch creation
#[deriving(Clone, Show)]
pub enum BatchError {
    /// Error connecting shader parameters
    ErrorParameters(ParameterError),
}

/// Abstract batch trait
pub trait Batch {
    /// Obtain information about the mesh, program, and state
    fn get_data(&self) -> (&Mesh, Slice, &ProgramHandle, &DrawState);
    /// Fill shader parameter values
    fn fill_params(&self, ::shade::ParamValues);
}

/// Heavy Batch - self-contained, but has heap-allocated data
pub struct HeavyBatch<L, T> {
    mesh: Mesh,
    #[allow(dead_code)]
    mesh_link: MeshLink,
    /// Mesh slice
    pub slice: Slice,
    program: ProgramHandle,
    param: T,
    param_link: L,
    /// Draw state
    pub state: DrawState,
}

impl<L, T: ShaderParam<L>> HeavyBatch<L, T> {
    /// Create a new heavy batch
    pub fn new(mesh: Mesh, program: ProgramHandle, param: T)
           -> Result<HeavyBatch<L, T>, BatchError> {
        let slice = mesh.get_slice(::device::TriangleList);
        let link = match ShaderParam::create_link(None::<T>, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(ErrorParameters(e))
        };
        Ok(HeavyBatch {
            mesh: mesh,
            mesh_link: MeshLink,
            slice: slice,
            program: program,
            param: param,
            param_link: link,
            state: DrawState::new(),
        })
    }
}

impl<'a, L, T: ShaderParam<L>> Batch for &'a HeavyBatch<L, T> {
    fn get_data(&self) -> (&Mesh, Slice, &ProgramHandle, &DrawState) {
        (&self.mesh, self.slice, &self.program, &self.state)
    }

    fn fill_params(&self, values: ::shade::ParamValues) {
        self.param.fill_params(&self.param_link, values);
    }
}

type Index = u16;

struct Id<T>(Index);

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
    fn find_or_insert(&mut self, value: &T) -> Id<T> {
        let i = self.data.iter().position(|v| v == value).unwrap_or_else(|| {
            let i = self.data.len();
            self.data.push(value.clone());
            i
        });
        Id(i as Index)
    }
}


/// Light Batch - copyable and smaller, but depends on the `Context`.
/// It has references to the resources (mesh, program, state), that are held
/// by the context that created the batch, so these have to be used together.
pub struct LightBatch<L, T> {
    mesh_id: Id<Mesh>,
    #[allow(dead_code)]
    mesh_link: MeshLink,
    slice: Slice,
    program_id: Id<ProgramHandle>,
    param_link: L,
    state_id: Id<DrawState>,
}

/// Factory of light batches, required to always be used with them.
pub struct Context {
    meshes: Array<Mesh>,
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
    /// Produce a new light batch
    pub fn batch<L, T: ShaderParam<L>>(&mut self, mesh: &Mesh, slice: Slice,
                program: &ProgramHandle, state: &DrawState)
                -> Result<LightBatch<L, T>, BatchError> {
        let link = match ShaderParam::create_link(None::<T>, program.get_info()) {
            Ok(l) => l,
            Err(e) => return Err(ErrorParameters(e))
        };
        Ok(LightBatch {
            mesh_id: self.meshes.find_or_insert(mesh),
            mesh_link: MeshLink,
            slice: slice,
            program_id: self.programs.find_or_insert(program),
            param_link: link,
            state_id: self.states.find_or_insert(state),
        })
    }
}

impl<'a, L, T: ShaderParam<L>> Batch for (&'a LightBatch<L, T>, &'a T, &'a Context) {
    fn get_data(&self) -> (&Mesh, Slice, &ProgramHandle, &DrawState) {
        let (b, _, ctx) = *self;
        (ctx.meshes.get(b.mesh_id),
        b.slice,
        ctx.programs.get(b.program_id),
        ctx.states.get(b.state_id))
    }

    fn fill_params(&self, values: ::shade::ParamValues) {
        let (b, data, _) = *self;
        data.fill_params(&b.param_link, values);
    }
}
