
use spirv_utils::{self, desc, instruction};
use gfx_core::shade;

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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SpirvReflection {
    entry_points: Vec<EntryPoint>,
    variables: Vec<Variable>,
}

pub fn reflect_spirv_module(code: &[u8]) -> SpirvReflection {
    use spirv_utils::instruction::Instruction;

    let module = spirv_utils::RawModule::read_module(code).unwrap();

    let mut entry_points = Vec::new();
    let mut variables = Vec::new();
    for inst in module.instructions() {
        match *inst {
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
                variables.push(Variable {
                    id: result_id.into(),
                    name: name.into(),
                    ty: result_type.into(),
                    storage_class: storage_class,
                    decoration: decoration,
                });
            },
            _ => (),
        }
    }

    println!("{:?}", entry_points);
    println!("{:?}", variables);

    SpirvReflection {
        entry_points: entry_points,
        variables: variables,
    }
}

pub fn populate_info(info: &mut shade::ProgramInfo, stage: shade::Stage, reflection: &SpirvReflection) {
    if stage == shade::Stage::Vertex {
        // record vertex attributes
        if let Some(entry_point) = reflection.entry_points.iter().find(|ep| ep.name == "main" && ep.stage == stage) {
            println!("{:?}", entry_point);
            for attrib in entry_point.interface.iter() {
                if let Some(var) = reflection.variables.iter().find(|var| var.id == *attrib && var.storage_class == desc::StorageClass::Input) {
                    let slot = var.decoration.iter().filter_map(|dec| match *dec {
                                    instruction::Decoration::Location(slot) => Some(slot),
                                    _ => None,
                                }).next().expect("Missing location decoration");
                }
            }
        }
    } else if stage == shade::Stage::Pixel {
        // record pixel outputs
        if let Some(entry_point) = reflection.entry_points.iter().find(|ep| ep.name == "main" && ep.stage == stage) {
            for attrib in entry_point.interface.iter() {
                if let Some(var) = reflection.variables.iter().find(|var| var.id == *attrib && var.storage_class == desc::StorageClass::Output) {

                }
            }
        }
    }
}
