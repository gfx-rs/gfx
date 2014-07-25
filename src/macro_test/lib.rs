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

#![crate_name = "gfx_macro_test"]

#![feature(phase)]

#[phase(plugin)]
extern crate gfx_macro;
extern crate gfx;

#[test]
fn test_vertex_format() {
    use a = gfx::attrib;
    #[packed]
    #[vertex_format]
    struct MyVertex {
         a0: [f32, ..2],
         #[normalized]
         a1: i16,
         #[as_float]
         a2: [i8, ..4],
         #[as_double]
         a3: f64,
    }

    let buf = 0 as gfx::BufferHandle;
    let mesh = gfx::Mesh::from::<MyVertex>(buf, 0);
    let stride = 22 as a::Stride;

    assert_eq!(mesh.attributes, vec![
        gfx::Attribute {
            buffer: buf,
            elem_count: 2,
            elem_type: a::Float(a::FloatDefault, a::F32),
            offset: 0,
            stride: stride,
            name: "a0".to_string(),
        },
        gfx::Attribute {
            buffer: buf,
            elem_count: 1,
            elem_type: a::Int(a::IntNormalized, a::U16, a::Signed),
            offset: 8,
            stride: stride,
            name: "a1".to_string(),
        },
        gfx::Attribute {
            buffer: buf,
            elem_count: 4,
            elem_type: a::Int(a::IntAsFloat, a::U8, a::Signed),
            offset: 10,
            stride: stride,
            name: "a2".to_string(),
        },
        gfx::Attribute {
            buffer: buf,
            elem_count: 1,
            elem_type: a::Float(a::FloatPrecision, a::F64),
            offset: 14,
            stride: stride,
            name: "a3".to_string(),
        }
    ]);
}

#[test]
fn test_shader_param() {
    #[shader_param]
    struct MyParam {
        a: i32,
        b: [f32, ..4],
        c: gfx::TextureHandle,
        d: gfx::BufferHandle,
    }
}
