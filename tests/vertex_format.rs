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

#![feature(plugin, custom_attribute)]
#![plugin(gfx_macros)]

mod secret_lib;

// Test all features
#[repr(packed)]
#[vertex_format]
#[derive(Copy)]
struct MyVertex {
    a0: [f32; 2],
    #[normalized]
    a1: i16,
    #[as_float]
    a2: [i8; 4],
    #[as_double]
    a3: f64,
    #[name = "a_a4"]
    a4: [f32; 3],
}

// Test that there are no conflicts between the two reexport modules
#[repr(packed)]
#[vertex_format]
#[derive(Copy)]
struct MyInstance {
    a0: [f32; 2],
}

#[test]
fn test_vertex_format() {
    use secret_lib::gfx::attrib::{Type, IntSubType, FloatSubType,
                                  IntSize, FloatSize, SignFlag,
                                  Format, Stride};
    use secret_lib::gfx;
    use secret_lib::gfx::device::{BufferInfo, BufferUsage, BufferHandle, HandleFactory};

    let info = BufferInfo {
        usage: BufferUsage::Static,
        size: 0,
    };
    let handle = ().make_handle((), info);
    let buf_vert: BufferHandle<(), MyVertex> = BufferHandle::from_raw(handle);
    let buf_inst: BufferHandle<(), MyInstance> = BufferHandle::from_raw(handle);
    let mesh = gfx::Mesh::from_format_instanced::<MyVertex, MyInstance>(buf_vert, 0, buf_inst);
    let stride_vert = 34 as Stride;
    let stride_inst = 8 as Stride;

    assert_eq!(mesh.attributes, vec![
        gfx::Attribute {
            name: "a0".to_string(),
            buffer: buf_vert.raw(),
            format: Format {
                elem_count: 2,
                elem_type: Type::Float(FloatSubType::Default, FloatSize::F32),
                offset: 0,
                stride: stride_vert,
                instance_rate: 0,
            },
        },
        gfx::Attribute {
            name: "a1".to_string(),
            buffer: buf_vert.raw(),
            format: Format {
                elem_count: 1,
                elem_type: Type::Int(IntSubType::Normalized, IntSize::U16, SignFlag::Signed),
                offset: 8,
                stride: stride_vert,
                instance_rate: 0,
            },
        },
        gfx::Attribute {
            name: "a2".to_string(),
            buffer: buf_vert.raw(),
            format: Format {
                elem_count: 4,
                elem_type: Type::Int(IntSubType::AsFloat, IntSize::U8, SignFlag::Signed),
                offset: 10,
                stride: stride_vert,
                instance_rate: 0,
            },
        },
        gfx::Attribute {
            name: "a3".to_string(),
            buffer: buf_vert.raw(),
            format: Format {
                elem_count: 1,
                elem_type: Type::Float(FloatSubType::Precision, FloatSize::F64),
                offset: 14,
                stride: stride_vert,
                instance_rate: 0,
            },
        },
        gfx::Attribute {
            name: "a_a4".to_string(),
            buffer: buf_vert.raw(),
            format: Format {
                elem_count: 3,
                elem_type: Type::Float(FloatSubType::Default, FloatSize::F32),
                offset: 22,
                stride: stride_vert,
                instance_rate: 0,
            },
        },
        gfx::Attribute {
            name: "a0".to_string(),
            buffer: buf_vert.raw(),
            format: Format {
                elem_count: 2,
                elem_type: Type::Float(FloatSubType::Default, FloatSize::F32),
                offset: 0,
                stride: stride_inst,
                instance_rate: 1,
            },
        },
    ]);
}
