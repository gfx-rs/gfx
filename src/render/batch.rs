#![allow(dead_code)]
#![allow(missing_doc)]
#![allow(unused_variable)]

use device::ProgramHandle;
use mesh::{Mesh, Slice};
use shade::{ParameterError, ShaderParam};
use state::DrawState;


/*-----------
Needs:
* light-weight, POD
* self-contained, safe
Variants:
1. Include ProgramInfo (pod?)
    1. Make it POD (how?)
    2. Clone instead (no pod)
2. Share ProgramInfo (no safe?)
    - context is required
    - also share the mesh
-----------*/

struct MeshLink;    //TODO

pub enum BatchError {
    ErrorParameters(ParameterError),
}

pub trait Batch {
    fn get_data(&self) -> (&Mesh, Slice, &ProgramHandle, &DrawState);
    fn fill_params(&self, ::shade::ParamValues);
}

/// Heavy Batch - self-contained
pub struct HeavyBatch<L, T> {
    mesh: Mesh,
    mesh_link: MeshLink,
    pub slice: Slice,
    program: ProgramHandle,
    param: T,
    param_link: L,
    pub state: DrawState,
}

impl<L, T: ShaderParam<L>> HeavyBatch<L, T> {
    fn new(mesh: Mesh, program: ProgramHandle, param: T, state: DrawState)
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
    generation: u16,
}

impl<T> Array<T> {
    fn new() -> Array<T> {
        Array {
            data: Vec::new(),
            generation: 0,
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


/// Light Batch - copyable and smaller
pub struct LightBatch<L> {
    mesh_id: Id<Mesh>,
    mesh_link: MeshLink,
    slice: Slice,
    program_id: Id<ProgramHandle>,
    param_link: L,
    state_id: Id<DrawState>,
}

/// Factory of light batches
pub struct Context {
    meshes: Array<Mesh>,
    programs: Array<ProgramHandle>,
    states: Array<DrawState>,
}

impl Context {
    pub fn new() -> Context {
        Context {
            meshes: Array::new(),
            programs: Array::new(),
            states: Array::new(),
        }
    }
}

impl Context {
    pub fn batch<L, T: ShaderParam<L>>(&mut self, mesh: &Mesh, slice: Slice,
                program: &ProgramHandle, state: &DrawState)
                -> Result<LightBatch<L>, BatchError> {
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

impl<'a, L, T: ShaderParam<L>> Batch for (&'a LightBatch<L>, &'a T, &'a Context) {
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
