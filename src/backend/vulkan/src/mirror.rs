
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
    Struct(Vec<Type>),
    Sampler,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Type {
    id: desc::Id,
    ty: Ty,
    decoration: Vec<instruction::Decoration>,
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

fn map_member_name_by_id<'a>(module: &'a spirv_utils::RawModule, id: desc::Id, member_id: u32) -> Option<&'a str> {
    module.uses(id).filter_map(|instr| {
        match *instr {
            instruction::Instruction::MemberName { ref name, member, .. } if member == member_id => {
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

fn map_member_decorations_by_id(module: &spirv_utils::RawModule, id: desc::Id, member_id: u32) -> Vec<instruction::Decoration> {
    module.uses(id).filter_map(|instr| {
        match *instr {
            instruction::Instruction::MemberDecorate { ref decoration, member, .. } if member == member_id => {
                Some(decoration.clone())
            },
            // TODO: GroupDecorations
            _ => None,
        }
    }).collect::<Vec<_>>()
}

fn map_image_to_texture_type(dim: desc::Dim, arrayed: bool, multisampled: bool) -> TextureType {
    use spirv_utils::desc::Dim;
    let arrayed = if arrayed { shade::IsArray::Array } else { shade::IsArray::NoArray };
    let multisampled = if multisampled { shade::IsMultiSample::MultiSample } else { shade::IsMultiSample::NoMultiSample };
    match dim {
        Dim::_1D => shade::TextureType::D1(arrayed),
        Dim::_2D => shade::TextureType::D2(arrayed, multisampled),
        Dim::_3D => shade::TextureType::D3,
        Dim::Cube => shade::TextureType::Cube(arrayed),
        Dim::Buffer => shade::TextureType::Buffer,

        _ => unimplemented!(),
    }
}

fn map_scalar_to_basetype(instr: &instruction::Instruction) -> Option<BaseType> {
    use spirv_utils::instruction::Instruction;
    match *instr {
        Instruction::TypeBool { .. } => Some(BaseType::Bool),
        Instruction::TypeInt { width: 32, signed: false, .. } => Some(BaseType::U32),
        Instruction::TypeInt { width: 32, signed: true, .. } => Some(BaseType::I32),
        Instruction::TypeFloat { width: 32, ..} => Some(BaseType::F32),
        Instruction::TypeFloat { width: 64, ..} => Some(BaseType::F64),

        _ => None,
    }
}

fn map_instruction_to_type(module: &spirv_utils::RawModule, instr: &instruction::Instruction) -> Option<Type> {
    use spirv_utils::instruction::{Decoration, Instruction};
    let id_ty = match *instr {
        Instruction::TypeBool { result_type } => {
            Some((result_type, Ty::Basic(map_scalar_to_basetype(instr).unwrap(), ContainerType::Single)))
        },
        Instruction::TypeInt { result_type, width: 32, signed: false } => {
            Some((result_type, Ty::Basic(map_scalar_to_basetype(instr).unwrap(), ContainerType::Single)))
        },
        Instruction::TypeInt { result_type, width: 32, signed: true } => {
            Some((result_type, Ty::Basic(map_scalar_to_basetype(instr).unwrap(), ContainerType::Single)))
        },
        Instruction::TypeFloat { result_type, width: 32 } => {
            Some((result_type, Ty::Basic(map_scalar_to_basetype(instr).unwrap(), ContainerType::Single)))
        },
        Instruction::TypeFloat { result_type, width: 64 } => {
            Some((result_type, Ty::Basic(map_scalar_to_basetype(instr).unwrap(), ContainerType::Single)))
        },
        Instruction::TypeVector { result_type, type_id, len } => {
            let comp_ty = module.def(type_id).unwrap();
            Some((result_type, Ty::Basic(map_scalar_to_basetype(comp_ty).unwrap(), ContainerType::Vector(len as u8))))
        },
        Instruction::TypeMatrix { result_type, type_id, cols } => {
            let (base, rows) = match *module.def(type_id).unwrap() {
                Instruction::TypeVector { type_id, len, .. } => {
                    let comp_ty = module.def(type_id).unwrap();
                    (map_scalar_to_basetype(comp_ty).unwrap(), len)
                },
                _ => unreachable!(), // SPIR-V module would be invalid
            };

            let decoration = map_decorations_by_id(&module, result_type.into());

            // NOTE: temporary value, changes might be needed later depending on the decorations of the variable
            let matrix_format = if decoration.iter().find(|deco| **deco == Decoration::RowMajor).is_some() {
                shade::MatrixFormat::RowMajor
            } else {
                shade::MatrixFormat::ColumnMajor
            };

            Some((result_type, Ty::Basic(base, ContainerType::Matrix(matrix_format, rows as u8, cols as u8))))
        },
        Instruction::TypeStruct { result_type, ref fields } => {
            Some((
                result_type,
                Ty::Struct(
                    fields.iter().filter_map(|field| // TODO: should be `map()`, currently to ignore unsupported types
                        map_instruction_to_type(module, module.def(*field).unwrap())
                    ).collect::<Vec<_>>()
                )
            ))
        },
        Instruction::TypeSampler { result_type } => {
            Some((result_type, Ty::Sampler))
        },
        Instruction::TypeImage { result_type, type_id, dim, arrayed, multisampled, .. } => {
            Some((result_type, Ty::Image(map_scalar_to_basetype(module.def(type_id).unwrap()).unwrap(), map_image_to_texture_type(dim, arrayed, multisampled))))
        },

        _ => None,
    };

    id_ty.map(|(id, ty)| Type {
            id: id.into(),
            ty: ty,
            decoration: map_decorations_by_id(&module, id.into()),
        })
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SpirvReflection {
    entry_points: Vec<EntryPoint>,
    variables: Vec<Variable>,
    types: Vec<Type>,
}

pub fn reflect_spirv_module(code: &[u8]) -> SpirvReflection {
    use spirv_utils::instruction::Instruction;

    let module = spirv_utils::RawModule::read_module(code).expect("Unable to parse SPIR-V module");

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
                } else {
                    error!("Unsupported execution model: {:?}", execution_model);
                }
            },
            Instruction::Variable { result_type, result_id, storage_class, .. } => {
                let decoration = map_decorations_by_id(&module, result_id.into());
                let ty = {
                    // remove indirection layer as the type of every variable is a OpTypePointer
                    let ptr_ty = module.def::<desc::TypeId>(result_type.into()).unwrap();
                    match *ptr_ty {
                        Instruction::TypePointer { ref pointee, .. } => *pointee,
                        _ => unreachable!(), // SPIR-V module would be invalid
                    }
                };

                // Every variable MUST have an name annotation
                // `glslang` seems to emit empty strings for uniforms with struct type,
                // therefore we need to retrieve the name from the struct decl
                let name = {
                    let name = map_name_by_id(&module, result_id.into()).expect("Missing name annotation");
                    if name.is_empty() {
                        map_name_by_id(&module, ty.into()).expect("Missing name annotation")
                    } else {
                        name
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

            _ => {
                // Reflect types, if we have OpTypeXXX
                if let Some(ty) = map_instruction_to_type(&module, instr) {
                    types.push(ty);
                }
            },
        }
    }

    SpirvReflection {
        entry_points: entry_points,
        variables: variables,
        types: types,
    }
}

pub fn populate_info(info: &mut shade::ProgramInfo, stage: shade::Stage, reflection: &SpirvReflection) {
    if stage == shade::Stage::Vertex {
        // record vertex attributes
        let entry_point = reflection.entry_points.iter().find(|ep| ep.name == "main" && ep.stage == stage).expect("Couln't find entry point!");
        for attrib in entry_point.interface.iter() {
            if let Some(var) = reflection.variables.iter().find(|var| var.id == *attrib) {
                if var.storage_class == desc::StorageClass::Input {
                    let attrib_name = var.name.clone();
                    let slot = var.decoration.iter()
                                     .find(|dec| if let &instruction::Decoration::Location(..) = *dec { true } else { false })
                                     .map(|dec| if let instruction::Decoration::Location(slot) = *dec { Some(slot) } else { None })
                                     .expect("Missing location decoration").unwrap();
                    let ty = reflection.types.iter().find(|ty| ty.id == var.ty).unwrap();
                    if let Ty::Basic(base, container) = ty.ty {
                        info.vertex_attributes.push(shade::AttributeVar {
                            name: attrib_name,
                            slot: slot as core::AttributeSlot,
                            base_type: base,
                            container: container,
                        });
                    } else {
                        error!("Unsupported type as vertex attribute: {:?}", ty.ty);
                    }
                }
            } else {
                error!("Missing vertex attribute reflection: {:?}", attrib);
            }
        }
    } else if stage == shade::Stage::Pixel {
        // record pixel outputs
        if let Some(entry_point) = reflection.entry_points.iter().find(|ep| ep.name == "main" && ep.stage == stage) {
            for out in entry_point.interface.iter() {
                if let Some(var) = reflection.variables.iter().find(|var| var.id == *out) {
                    if var.storage_class == desc::StorageClass::Output {
                        let target_name = var.name.clone();
                        let slot = var.decoration.iter()
                                         .find(|dec| if let &instruction::Decoration::Location(..) = *dec { true } else { false })
                                         .map(|dec| if let instruction::Decoration::Location(slot) = *dec { Some(slot) } else { None })
                                         .expect("Missing location decoration").unwrap();
                        let ty = reflection.types.iter().find(|ty| ty.id == var.ty).unwrap();
                        if let Ty::Basic(base, container) = ty.ty {
                            info.outputs.push(shade::OutputVar {
                                name: target_name,
                                slot: slot as core::ColorSlot,
                                base_type: base,
                                container: container,
                            });
                        } else {
                            error!("Unsupported type as pixel shader output: {:?}", ty.ty);
                        }
                    }
                } else {
                    error!("Missing pixel shader output reflection: {:?}", out);
                }
            }
        }
    }

    // Handle resources
    // We use only one descriptor set currently
    for var in reflection.variables.iter() {
        use spirv_utils::desc::StorageClass::*;
        match var.storage_class {
            Uniform | UniformConstant => {
                if let Some(ty) = reflection.types.iter().find(|ty| ty.id == var.ty) {
                    // constant buffers
                    match ty.ty {
                        Ty::Struct(ref fields) => {
                            let elements = Vec::new();
                            for field in fields {
                                // TODO:
                            }

                            let buffer_name = var.name.clone();
                            let slot = var.decoration.iter()
                                             .find(|dec| if let &instruction::Decoration::Binding(..) = *dec { true } else { false })
                                             .map(|dec| if let instruction::Decoration::Binding(slot) = *dec { Some(slot) } else { None })
                                             .expect("Missing binding decoration").unwrap();
                            info.constant_buffers.push(shade::ConstantBufferVar {
                                name: buffer_name,
                                slot: slot as core::ConstantBufferSlot,
                                size: 0, // TODO:
                                usage: shade::VERTEX | shade::GEOMETRY | shade::PIXEL, // TODO:
                                elements: elements,
                            });
                        },

                        Ty::Sampler => {
                            let sampler_name = var.name.trim_right_matches('_');
                            let slot = var.decoration.iter()
                                             .find(|dec| if let &instruction::Decoration::Binding(..) = *dec { true } else { false })
                                             .map(|dec| if let instruction::Decoration::Binding(slot) = *dec { Some(slot) } else { None })
                                             .expect("Missing binding decoration").unwrap();
                            info.samplers.push(shade::SamplerVar {
                                name: sampler_name.to_owned(),
                                slot: slot as core::SamplerSlot,
                                ty: shade::SamplerType(shade::IsComparison::NoCompare, shade::IsRect::NoRect), // TODO:
                                usage: shade::VERTEX | shade::GEOMETRY | shade::PIXEL, // TODO:
                            });
                        },

                        Ty::Image(base_type, texture_type) => {
                            let texture_name = var.name.clone();
                            let slot = var.decoration.iter()
                                             .find(|dec| if let &instruction::Decoration::Binding(..) = *dec { true } else { false })
                                             .map(|dec| if let instruction::Decoration::Binding(slot) = *dec { Some(slot) } else { None })
                                             .expect("Missing binding decoration").unwrap();
                            info.textures.push(shade::TextureVar {
                                name: texture_name,
                                slot: slot as core::ResourceViewSlot,
                                base_type: base_type,
                                ty: texture_type,
                                usage: shade::VERTEX | shade::GEOMETRY | shade::PIXEL, // TODO:
                            });
                        },

                        _ => (),
                    }
                } else {
                    error!("Unsupported uniform type: {:?}", var.ty);
                }
            },
            _ => (),
        }
    }
}
