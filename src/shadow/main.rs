// Copyright 2015 The GFX developers.
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

extern crate cgmath;
extern crate genmesh;
#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

use gfx::attrib::Floater;
use gfx::traits::*;

gfx_vertex!( Vertex {
    a_Pos@ pos: [Floater<i8>; 3],
    a_TexCoord@ tex_coord: [Floater<u8>; 2],
});

impl Vertex {
    fn new(p: [i8; 3], t: [u8; 2]) -> Vertex {
        Vertex {
            pos: Floater::cast3(p),
            tex_coord: Floater::cast2(t),
        }
    }
}

gfx_parameters!( Params {
    u_Transform@ transform: [[f32; 4]; 4],
    u_Color@ color: [f32; 4],
});


fn create_mesh<R: gfx::Resources, F: gfx::Factory<R>>(factory: &mut F)
               -> (gfx::Mesh<R>, gfx::Slice<R>)
{
    let vertex_data = [
        // top (0, 0, 1)
        Vertex::new([-1, -1,  1], [0, 0]),
        Vertex::new([ 1, -1,  1], [1, 0]),
        Vertex::new([ 1,  1,  1], [1, 1]),
        Vertex::new([-1,  1,  1], [0, 1]),
        // bottom (0, 0, -1)
        Vertex::new([-1,  1, -1], [1, 0]),
        Vertex::new([ 1,  1, -1], [0, 0]),
        Vertex::new([ 1, -1, -1], [0, 1]),
        Vertex::new([-1, -1, -1], [1, 1]),
        // right (1, 0, 0)
        Vertex::new([ 1, -1, -1], [0, 0]),
        Vertex::new([ 1,  1, -1], [1, 0]),
        Vertex::new([ 1,  1,  1], [1, 1]),
        Vertex::new([ 1, -1,  1], [0, 1]),
        // left (-1, 0, 0)
        Vertex::new([-1, -1,  1], [1, 0]),
        Vertex::new([-1,  1,  1], [0, 0]),
        Vertex::new([-1,  1, -1], [0, 1]),
        Vertex::new([-1, -1, -1], [1, 1]),
        // front (0, 1, 0)
        Vertex::new([ 1,  1, -1], [1, 0]),
        Vertex::new([-1,  1, -1], [0, 0]),
        Vertex::new([-1,  1,  1], [0, 1]),
        Vertex::new([ 1,  1,  1], [1, 1]),
        // back (0, -1, 0)
        Vertex::new([ 1, -1,  1], [0, 0]),
        Vertex::new([-1, -1,  1], [1, 0]),
        Vertex::new([-1, -1, -1], [1, 1]),
        Vertex::new([ 1, -1, -1], [0, 1]),
    ];

    let mesh = factory.create_mesh(&vertex_data);

    let index_data: &[u8] = &[
         0,  1,  2,  2,  3,  0, // top
         4,  5,  6,  6,  7,  4, // bottom
         8,  9, 10, 10, 11,  8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    let slice = index_data.to_slice(factory, gfx::PrimitiveType::TriangleList);

    (mesh, slice)
}

//----------------------------------------

pub fn main() {
    use cgmath::{FixedArray, Matrix};

    let (mut stream, mut device, mut factory) = gfx_window_glutin::init(
        glutin::WindowBuilder::new()
            .with_title("Multi-threaded shadow rendering example with gfx-rs".to_string())
            .with_dimensions(800, 600)
            .with_gl(glutin::GL_CORE)
            .build().unwrap()
    );

    let program = {
        let vs = gfx::ShaderSource {
            glsl_120: Some(include_bytes!("shader/cube_120.glslv")),
            glsl_150: Some(include_bytes!("shader/cube_150.glslv")),
            .. gfx::ShaderSource::empty()
        };
        let fs = gfx::ShaderSource {
            glsl_120: Some(include_bytes!("shader/cube_120.glslf")),
            glsl_150: Some(include_bytes!("shader/cube_150.glslf")),
            .. gfx::ShaderSource::empty()
        };
        factory.link_program_source(vs, fs).unwrap()
    };

    let view = cgmath::Matrix4::look_at(
        &cgmath::Point3::new(1.5f32, -5.0, 3.0),
        &cgmath::Point3::new(0f32, 0.0, 0.0),
        &cgmath::Vector3::unit_z(),
    );
    let proj = cgmath::perspective(cgmath::deg(45.0f32),
                                   stream.get_aspect_ratio(), 1.0, 10.0);

    let data = Params {
        transform: proj.mul_m(&view).into_fixed(),
        color: [1.0, 1.0, 1.0, 1.0],
        _r: std::marker::PhantomData,
    };

    let (mesh, slice) = create_mesh(&mut factory);
    let mut batch = gfx::batch::OwnedBatch::new(mesh, program, data).unwrap();
    batch.slice = slice;
    batch.state.depth(gfx::state::Comparison::LessEqual, true);

    'main: loop {
        // quit when Esc is pressed.
        for event in stream.out.window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) => break 'main,
                glutin::Event::Closed => break 'main,
                _ => {},
            }
        }

        stream.clear(gfx::ClearData {
            color: [0.3, 0.3, 0.3, 1.0],
            depth: 1.0,
            stencil: 0,
        });
        stream.draw(&batch).unwrap();
        stream.present(&mut device);
    }
}
