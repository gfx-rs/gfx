
use spirv_utils::{self, desc, instruction};
use core;
use core::shade::{self, BaseType, ContainerType, TextureType};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Variable {
    id: desc::Id,
    name: String,
    ty: desc::Id,
    storage_class: desc::StorageClass,
    decoration: Vec<instruction::Decoration>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EntryPoint {
    name: String,
    stage: shade::Stage,
    interface: Box<[desc::Id]>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Ty {
    Basic(BaseType, ContainerType),
    Image(BaseType, TextureType),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Type {
    id: desc::Id,
    ty: Ty,
}

fn map_execution_model_to_stage(model: desc::ExecutionModel) -> Option<shade::Stage> {
    use spirv_utils::desc::ExecutionModel::*;
    match model {
        Vertex => Some(shade::Stage::Vertex),
        Geometry => Some(shade::Stage::Geometry),
        Fragment => Some(shade::Stage::Pixel),

        _ => None,
    }
}

fn map_name_by_id<'a>(module: &'a spirv_utils::RawModule, id: desc::Id) -> Option<&'a str> {
    module.uses(id).filter_map(|instr| {
        match *instr {
            instruction::Instruction::Name { ref name, .. } => {
                Some(&name[..])
            },
            _ => None,
        }
    }).next()
}

fn map_decorations_by_id(module: &spirv_utils::RawModule, id: desc::Id) -> Vec<instruction::Decoration> {
    module.uses(id).filter_map(|instr| {
        match *instr {
            instruction::Instruction::Decorate { ref decoration, .. } => {
                Some(decoration.clone())
            },
            // TODO: GroupDecorations
            _ => None,
        }
    }).collect::<Vec<_>>()
}

fn map_scalar_to_basetype(instr: &instruction::Instruction) -> Option<BaseType> {
    use spirv_utils::instruction::Instruction;
    match *instr {
        Instruction::TypeBool { result_type } => Some(BaseType::Bool),
        Instruction::TypeInt { result_type, width: 32, signed: false } => Some(BaseType::U32),
        Instruction::TypeInt { result_type, width: 32, signed: true } => Some(BaseType::I32),
        Instruction::TypeFloat { result_type, width: 32 } => Some(BaseType::F32),
        Instruction::TypeFloat { result_type, width: 64 } => Some(BaseType::F64),

        _ => None,
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SpirvReflection {
    entry_points: Vec<EntryPoint>,
    variables: Vec<Variable>,
    types: Vec<Type>,
}

pub fn reflect_spirv_module(code: &[u8]) -> SpirvReflection {
    use spirv_utils::instruction::Instruction;

    let module = spirv_utils::RawModule::read_module(code).unwrap();

    let mut entry_points = Vec::new();
    let mut variables = Vec::new();
    let mut types = Vec::new();
    for instr in module.instructions() {
        match *instr {
            Instruction::EntryPoint { execution_model, ref name, ref interface, .. } => {
                if let Some(stage) = map_execution_model_to_stage(execution_model) {
                    entry_points.push(EntryPoint {
                        name: name.clone(),
                        stage: stage,
                        interface: interface.clone(),
                    });
                }
            },
            Instruction::Variable { result_type, result_id, storage_class, .. } => {
                let name = map_name_by_id(&module, result_id.into()).unwrap(); // every variable MUST have an name annotation
                let decoration = map_decorations_by_id(&module, result_id.into());
                let ty = {
                    // remove indirection layer as the type of every variable is a OpTypePointer
                    let ptr_ty = module.def::<desc::TypeId>(result_type.into()).unwrap();
                    match *ptr_ty {
                        Instruction::TypePointer { ref pointee, .. } => *pointee,
                        _ => unreachable!(), // SPIR-V module would be invalid
                    }
                };

                variables.push(Variable {
                    id: result_id.into(),
                    name: name.into(),
                    ty: ty.into(),
                    storage_class: storage_class,
                    decoration: decoration,
                });
            },

            // Reflect types
            Instruction::TypeBool { result_type } => {
                types.push(Type {
                    id: result_type.into(),
                    ty: Ty::Basic(BaseType::Bool, ContainerType::Single),
                })
            },
            Instruction::TypeInt { result_type, width: 32, signed: false } => {
                types.push(Type {
                    id: result_type.into(),
                    ty: Ty::Basic(BaseType::U32, ContainerType::Single),
                })
            },
            Instruction::TypeInt { result_type, width: 32, signed: true } => {
                types.push(Type {
                    id: result_type.into(),
                    ty: Ty::Basic(BaseType::I32, ContainerType::Single),
                })
            },
            Instruction::TypeFloat { result_type, width: 32 } => {
                types.push(Type {
                    id: result_type.into(),
                    ty: Ty::Basic(BaseType::F32, ContainerType::Single),
                })
            },
            Instruction::TypeFloat { result_type, width: 64 } => {
                types.push(Type {
                    id: result_type.into(),
                    ty: Ty::Basic(BaseType::F64, ContainerType::Single),
                })
            },
            Instruction::TypeVector { result_type, type_id, len } => {
                let comp_ty = module.def(type_id).unwrap();
                types.push(Type {
                    id: result_type.into(),
                    ty: Ty::Basic(map_scalar_to_basetype(comp_ty).unwrap(), ContainerType::Vector(len as u8)),
                })
            },
            _ => (),
        }
    }

    println!("{:?}", entry_points);
    println!("{:?}", variables);

    SpirvReflection {
        entry_points: entry_points,
        variables: variables,
        types: types,
    }
}

pub fn populate_info(info: &mut shade::ProgramInfo, stage: shade::Stage, reflection: &SpirvReflection) {
    if stage == shade::Stage::Vertex {
        // record vertex attributes
        if let Some(entry_point) = reflection.entry_points.iter().find(|ep| ep.name == "main" && ep.stage == stage) {
            println!("{:?}", entry_point);
            for attrib in entry_point.interface.iter() {
                if let Some(var) = reflection.variables.iter().find(|var| var.id == *attrib && var.storage_class == desc::StorageClass::Input) {
                    let attrib_name = var.name.clone();
                    let slot = var.decoration.iter().filter_map(|dec| match *dec {
                                    instruction::Decoration::Location(slot) => Some(slot),
                                    _ => None,
                                }).next().expect("Missing location decoration");

                    let ty = reflection.types.iter().find(|ty| ty.id == var.ty).unwrap();
                    if let Ty::Basic(base, container) = ty.ty {
                        info.vertex_attributes.push(shade::AttributeVar {
                            name: attrib_name,
                            slot: slot as core::AttributeSlot,
                            base_type: base,
                            container: container,
                        });
                    }
                }
            }
        }
    } else if stage == shade::Stage::Pixel {
        // record pixel outputs
        if let Some(entry_point) = reflection.entry_points.iter().find(|ep| ep.name == "main" && ep.stage == stage) {
            for attrib in entry_point.interface.iter() {
                if let Some(var) = reflection.variables.iter().find(|var| var.id == *attrib && var.storage_class == desc::StorageClass::Output) {
                    // TODO:
                }
            }
        }
    }

    // TODO: handle other resources
}
