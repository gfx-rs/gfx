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

#![feature(core, env, plugin, std_misc, custom_attribute)]
#![plugin(gfx_macros)]

extern crate cgmath;
extern crate gfx;
extern crate glfw;
extern crate time;
extern crate "gfx_gl" as gl;

use time::precise_time_s;
use cgmath::FixedArray;
use cgmath::{Matrix, Point3, Vector3, Matrix3, ToMatrix4};
use cgmath::{Transform, AffineMatrix3, Vector4, Array1};
use gfx::{Device, DeviceExt, ToSlice};
use gfx::batch::RefBatch;
use glfw::Context;
use gl::Gl;
use gl::types::*;
use std::mem;
use std::ptr;
use std::str;
use std::env;
use std::str::FromStr;
use std::sync::mpsc::Receiver;
use std::iter::repeat;
use std::ffi::CString;
use std::num::Int;

#[vertex_format]
#[derive(Copy)]
struct Vertex {
    #[as_float]
    #[name = "a_Pos"]
    pos: [i8; 3],
}

// The shader_param attribute makes sure the following struct can be used to
// pass parameters to a shader.
#[shader_param]
struct Params {
    #[name = "u_Transform"]
    transform: [[f32; 4]; 4],
}

static VERTEX_SRC: &'static [u8] = b"
    #version 150 core
    in vec3 a_Pos;
    uniform mat4 u_Transform;

    void main() {
        gl_Position = u_Transform * vec4(a_Pos, 1.0);
    }
";

static FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core
    out vec4 o_Color;

    void main() {
        o_Color = vec4(1., 0., 0., 1.);
    }
";

//----------------------------------------

fn gfx_main(mut glfw: glfw::Glfw,
            mut window: glfw::Window,
            events: Receiver<(f64, glfw::WindowEvent)>,
            dimension: i16) {
    let (w, h) = window.get_framebuffer_size();
    let frame = gfx::Frame::new(w as u16, h as u16);

    let mut device = gfx::GlDevice::new(|s| window.get_proc_address(s));

    let state = gfx::DrawState::new().depth(gfx::state::Comparison::LessEqual, true);

    let vertex_data = [
        // front (0, 1, 0)
        Vertex { pos: [-1,  1, -1] },
        Vertex { pos: [ 1,  1, -1] },
        Vertex { pos: [ 1,  1,  1] },
    ];

    let mesh = device.create_mesh(&vertex_data);
    let slice = mesh.to_slice(gfx::PrimitiveType::TriangleList);

    let texture_info = gfx::tex::TextureInfo {
        width: 1,
        height: 1,
        depth: 1,
        levels: 1,
        kind: gfx::tex::TextureKind::Texture2D,
        format: gfx::tex::RGBA8,
    };
    let image_info = texture_info.to_image_info();
    let texture = device.create_texture(texture_info)
        .ok().expect("Failed to create texture");
    device.update_texture(&texture, &image_info,
                          &[0x20u8, 0xA0u8, 0xC0u8, 0x00u8])
        .ok().expect("Failed to update texture");

    let program = device.link_program(VERTEX_SRC, FRAGMENT_SRC)
                        .ok().expect("Failed to link shaders");
    let view: AffineMatrix3<f32> = Transform::look_at(
        &Point3::new(0f32, -5.0, 0.0),
        &Point3::new(0f32, 0.0, 0.0),
        &Vector3::unit_z(),
    );
    let aspect = w as f32 / h as f32;
    let proj = cgmath::perspective(cgmath::deg(45.0f32), aspect, 1.0, 10.0);

    let clear_data = gfx::ClearData {
        color: [0.3, 0.3, 0.3, 1.0],
        depth: 1.0,
        stencil: 0,
    };

    let mut graphics = gfx::Graphics::new(device);
    let batch: RefBatch<Params> = graphics.make_batch(&program, &mesh, slice, &state)
                                          .ok().expect("Failed to make batch");

    while !window.should_close() {
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Key(glfw::Key::Escape, _, glfw::Action::Press, _) =>
                    window.set_should_close(true),
                _ => {},
            }
        }

        let start = precise_time_s() * 1000.;
        graphics.clear(clear_data, gfx::COLOR | gfx::DEPTH, &frame);

        for x in (-dimension) ..dimension {
            for y in (-dimension) ..dimension {
                let mut model = Matrix3::identity().mul_s(0.01f32).to_matrix4();
                model.w = Vector4::new(x as f32 * 0.05,
                                       0f32,
                                       y as f32 * 0.05,
                                       1f32);

                let data = Params {
                    transform: proj.mul_m(&view.mat)
                                   .mul_m(&model).into_fixed(),
                };
                graphics.draw(&batch, &data, &frame).unwrap();
            }
        }

        let pre_submit = precise_time_s() * 1000.;
        graphics.end_frame();
        let post_submit = precise_time_s() * 1000.;
        window.swap_buffers();
        let swap = precise_time_s() * 1000.;

        println!("total time:\t\t{0:4.2}ms", swap - start);
        println!("\tcreate list:\t{0:4.2}ms", pre_submit - start);
        println!("\tsubmit:\t\t{0:4.2}ms", post_submit - pre_submit);
        println!("\tgpu wait:\t{0:4.2}ms", swap - post_submit)
    }
}

static VS_SRC: &'static str = "
    #version 150 core
    in vec3 a_Pos;
    uniform mat4 u_Transform;

    void main() {
        gl_Position = u_Transform * vec4(a_Pos, 1.0);
    }
";


static FS_SRC: &'static str = "
    #version 150 core
    out vec4 o_Color;

    void main() {
        o_Color = vec4(1., 0., 0., 1.);
    }
";


fn compile_shader(gl: &Gl, src: &str, ty: GLenum) -> GLuint { unsafe {
    use std::num::Int;
    let shader = gl.CreateShader(ty);
    // Attempt to compile the shader
    let src = CString::from_slice(src.as_bytes());
    gl.ShaderSource(shader, 1, &src.as_ptr(), ptr::null());
    gl.CompileShader(shader);

    // Get the compile status
    let mut status = gl::FALSE as GLint;
    gl.GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

    // Fail on error
    if status != (gl::TRUE as GLint) {
        let mut len: GLint = 0;
        gl.GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
        let mut buf: Vec<u8> = repeat(0u8).take((len as isize).saturating_sub(1) as usize).collect();     // subtract 1 to skip the trailing null character
        gl.GetShaderInfoLog(shader, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
        panic!("{}", str::from_utf8(buf.as_slice()).ok().expect("ShaderInfoLog not valid utf8"));
    }
    shader
}}

fn link_program(gl: &Gl, vs: GLuint, fs: GLuint) -> GLuint { unsafe {
    let program = gl.CreateProgram();
    gl.AttachShader(program, vs);
    gl.AttachShader(program, fs);
    gl.LinkProgram(program);
    // Get the link status
    let mut status = gl::FALSE as GLint;
    gl.GetProgramiv(program, gl::LINK_STATUS, &mut status);

    // Fail on error
    if status != (gl::TRUE as GLint) {
        let mut len: GLint = 0;
        gl.GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
        let mut buf: Vec<u8> = repeat(0u8).take((len as isize).saturating_sub(1) as usize).collect();     // subtract 1 to skip the trailing null character
        gl.GetProgramInfoLog(program, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
        panic!("{}", str::from_utf8(buf.as_slice()).ok().expect("ProgramInfoLog not valid utf8"));
    }
    program
}}

fn gl_main(mut glfw: glfw::Glfw,
           mut window: glfw::Window,
           _: Receiver<(f64, glfw::WindowEvent),>,
           dimension: i16) {
    let gl = Gl::load_with(|s| window.get_proc_address(s));

    // Create GLSL shaders
    let vs = compile_shader(&gl, VS_SRC, gl::VERTEX_SHADER);
    let fs = compile_shader(&gl, FS_SRC, gl::FRAGMENT_SHADER);
    let program = link_program(&gl, vs, fs);

    let mut vao = 0;
    let mut vbo = 0;

    let trans_uniform = unsafe {
        // Create Vertex Array Object
        gl.GenVertexArrays(1, &mut vao);
        gl.BindVertexArray(vao);

        // Create a Vertex Buffer Object and copy the vertex data to it
        gl.GenBuffers(1, &mut vbo);
        gl.BindBuffer(gl::ARRAY_BUFFER, vbo);

        let vertex_data = vec![
            // front (0, 1, 0)
            Vertex { pos: [-1,  1, -1] },
            Vertex { pos: [ 1,  1, -1] },
            Vertex { pos: [ 1,  1,  1] },
        ];

        gl.BufferData(gl::ARRAY_BUFFER,
                      (vertex_data.len() * mem::size_of::<Vertex>()) as GLsizeiptr,
                      mem::transmute(&vertex_data[0]),
                      gl::STATIC_DRAW);

        // Use shader program
        gl.UseProgram(program);
        let o_color = CString::from_slice("o_Color".as_bytes());
        gl.BindFragDataLocation(program, 0, o_color.as_ptr());

        // Specify the layout of the vertex data
        let a_pos = CString::from_slice("a_Pos".as_bytes());
        gl.BindFragDataLocation(program, 0, a_pos.as_ptr());

        let pos_attr = gl.GetAttribLocation(program, a_pos.as_ptr());
        gl.EnableVertexAttribArray(pos_attr as GLuint);
        gl.VertexAttribPointer(pos_attr as GLuint, 3, gl::BYTE,
                                gl::FALSE as GLboolean, 0, ptr::null());


        let u_transform = CString::from_slice("u_Transform".as_bytes());
        gl.GetUniformLocation(program, u_transform.as_ptr())
    };

    let (w, h) = window.get_framebuffer_size();
    let view: AffineMatrix3<f32> = Transform::look_at(
        &Point3::new(0f32, -5.0, 0.0),
        &Point3::new(0f32, 0.0, 0.0),
        &Vector3::unit_z(),
    );
    let aspect = w as f32 / h as f32;
    let proj = cgmath::perspective(cgmath::deg(45.0f32), aspect, 1.0, 10.0);

    while !window.should_close() {
        // Poll events
        glfw.poll_events();

        let start = precise_time_s() * 1000.;

        // Clear the screen to black
        unsafe {
            gl.ClearColor(0.3, 0.3, 0.3, 1.0);
            gl.Clear(gl::COLOR_BUFFER_BIT);
        }

        for x in (-dimension) ..dimension {
            for y in (-dimension) ..dimension {
                let mut model = Matrix3::identity().mul_s(0.01f32).to_matrix4();
                model.w = Vector4::new(x as f32 * 0.05,
                                       0f32,
                                       y as f32 * 0.05,
                                       1f32);

                let mat = proj.mul_m(&view.mat).mul_m(&model);

                unsafe {
                    gl.UniformMatrix4fv(trans_uniform,
                                        1,
                                        gl::FALSE,
                                        mat.x.ptr());
                    gl.DrawArrays(gl::TRIANGLES, 0, 3);
                }

            }
        }

        let submit = precise_time_s() * 1000.;

        // Swap buffers
        window.swap_buffers();
        let swap = precise_time_s() * 1000.;

        println!("total time:\t\t{0:4.2}ms", swap - start);
        println!("\tsubmit:\t\t{0:4.2}ms", submit - start);
        println!("\tgpu wait:\t{0:4.2}ms", swap - submit)

    }

    // Cleanup
    unsafe {
        gl.DeleteProgram(program);
        gl.DeleteShader(fs);
        gl.DeleteShader(vs);
        gl.DeleteBuffers(1, &vbo);
        gl.DeleteVertexArrays(1, &vao);
    }
}

fn main() {
    use std::num::Float;

    let ref mut args = env::args();
    let args_count = env::args().count();
    if args_count == 1 {
        println!("gfx-perf [gl|gfx] <size>");
        return;
    }

    let mode = args.nth(1).unwrap();
    let count: i16 = if args_count == 3 {
        FromStr::from_str(&args.next().unwrap()).ok()
    } else {
        None
    }.unwrap_or(10000);

    let count = ((count as f64).sqrt() / 2.) as i16;

    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)
        .ok().expect("Failed to initialize glfs-rs");

    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenglForwardCompat(true));
    glfw.window_hint(glfw::WindowHint::OpenglProfile(glfw::OpenGlProfileHint::Core));

    let (mut window, events) = glfw
        .create_window(640, 480, "Cube example", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.make_current();
    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    window.set_key_polling(true);

    println!("count is {}", count*count*4);
    match mode.as_slice() {
        "gfx" => gfx_main(glfw, window, events, count),
        "gl" => gl_main(glfw, window, events, count),
        x => {
            println!("{} is not a known mode", x)
        }
    }
}
