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


use cocoa::base::{selector, class};
use cocoa::foundation::{NSUInteger};

use objc_foundation::{INSDictionary, NSDictionary, INSArray, NSArray};

use gfx_core;
use gfx_core::shade;

use metal::*;

fn map_base_type_from_component(ct: MTLDataType) -> shade::BaseType {
    use metal::MTLDataType::*;

    match ct {
        Float | Float2 | Float3 | Float4 |
        Float2x2 | Float2x3 | Float2x4 |
        Float3x2 | Float3x3 | Float3x4 |
        Float4x2 | Float4x3 | Float4x4 => shade::BaseType::F32,
        _ => shade::BaseType::I32
    }
}

pub fn populate_info(info: &mut shade::ProgramInfo, stage: shade::Stage,
                     args: NSArray<MTLArgument>) {
    use gfx_core::shade::Stage;

    match stage {
        Stage::Vertex => {

        },
        Stage::Pixel => {

        },
        _ => {}
    }
}
