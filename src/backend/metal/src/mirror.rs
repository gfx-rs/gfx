// Copyright 2016 The Gfx-rs Developers.
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


//use cocoa::base::{selector, class};
//use cocoa::foundation::{NSUInteger};

use core::{self, shade};
use factory::DUMMY_BUFFER_SLOT;

use metal::*;

fn _map_base_type_from_component(ct: MTLDataType) -> shade::BaseType {
    use metal::MTLDataType::*;

    match ct {
        Float | Float2 | Float3 | Float4 |
        Float2x2 | Float2x3 | Float2x4 |
        Float3x2 | Float3x3 | Float3x4 |
        Float4x2 | Float4x3 | Float4x4 => shade::BaseType::F32,
        _ => shade::BaseType::I32
    }
}

pub fn populate_vertex_attributes(info: &mut shade::ProgramInfo, desc: NSArray<MTLVertexAttribute>) {
    use map::{map_base_type, map_container_type};

    for idx in 0..desc.count() {
        let attr = desc.object_at(idx);

        info.vertex_attributes.push(shade::AttributeVar {
            name: attr.name().into(),
            slot: attr.attribute_index() as core::AttributeSlot,
            base_type: map_base_type(attr.attribute_type()),
            container: map_container_type(attr.attribute_type()),
        });
    }
}

pub fn populate_info(info: &mut shade::ProgramInfo, stage: shade::Stage,
                     args: NSArray<MTLArgument>) {
//    use core::shade::Stage;
    use map::{map_base_type, map_texture_type};

    let usage = stage.into();

    for idx in 0..args.count() {
        let arg = args.object_at(idx);
        let name = arg.name();

        match arg.type_() {
            MTLArgumentType::Buffer => {
                // we skip the dummy buffer, which is used in our fake PSO
                if arg.index() == DUMMY_BUFFER_SLOT { continue; }

                info.constant_buffers.push(shade::ConstantBufferVar {
                    name: name.into(),
                    slot: arg.index() as core::ConstantBufferSlot,
                    size: arg.buffer_data_size() as usize,
                    usage: usage,
                    elements: Vec::new(), //TODO!
                });
            },
            MTLArgumentType::Texture => {
                info.textures.push(shade::TextureVar {
                    name: name.into(),
                    slot: arg.index() as core::ResourceViewSlot,
                    base_type: map_base_type(arg.texture_data_type()),
                    ty: map_texture_type(arg.texture_type()),
                    usage: usage,
                });
            },
            MTLArgumentType::Sampler => {
                let name = name.trim_right_matches('_');

                info.samplers.push(shade::SamplerVar {
                    name: name.into(),
                    slot: arg.index() as core::SamplerSlot,
                    ty: shade::SamplerType(shade::IsComparison::NoCompare,
                                           shade::IsRect::NoRect),
                    usage: usage,
                });
            },
            _ => {}

        }
    }
}
