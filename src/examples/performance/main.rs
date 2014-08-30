#![feature(phase)]
#![crate_name = "gfx-perf"]
#![feature(globs)]

extern crate cgmath;
extern crate gfx;
#[phase(plugin)]
extern crate gfx_macros;
extern crate glfw;
extern crate native;
extern crate time;
extern crate gl;

use time::precise_time_s;
use cgmath::FixedArray;
use cgmath::{Matrix, Point3, Vector3, Matrix3, ToMatrix4};
use cgmath::{Transform, AffineMatrix3, Vector4, Array1};
use gfx::{Device, DeviceHelper};
use glfw::Context;
use gl::types::*;
use std::mem;
use std::ptr;
use std::str;
use std::os;
use std::from_str::FromStr;

#[vertex_format]
struct Vertex {
    #[as_float]
    #[name = "a_Pos"]
    pos: [i8, ..3],
}

// The shader_param attribute makes sure the following struct can be used to
// pass parameters to a shader. Its argument is the name of the type that will
// be generated to represent your the program. Search for link_program below, to
// see how it's used.
#[shader_param(TriangleBatch)]
struct Params {
    #[name = "u_Transform"]
    transform: [[f32, ..4], ..4],
}

static VERTEX_SRC: gfx::ShaderSource = shaders! {
GLSL_150: b"
    #version 150 core
    in vec3 a_Pos;
    uniform mat4 u_Transform;

    void main() {
        gl_Position = u_Transform * vec4(a_Pos, 1.0);
    }
"
};

static FRAGMENT_SRC: gfx::ShaderSource = shaders! {
GLSL_150: b"
    #version 150 core
    out vec4 o_Color;

    void main() {
        o_Color = vec4(1., 0., 0., 1.);
    }
"
};

//----------------------------------------

// We need to run on the main thread, so ensure we are using the `native` runtime. This is
// technically not needed, since this is the default, but it's not guaranteed.
#[start]
fn start(argc: int, argv: *const *const u8) -> int {
     native::start(argc, argv, main)
}

fn gfx_main(glfw: glfw::Glfw,
            window: glfw::Window,
            events: Receiver<(f64, glfw::WindowEvent)>,
            dimension: int) {
    let (w, h) = window.get_framebuffer_size();
    let frame = gfx::Frame::new(w as u16, h as u16);

    let mut device = gfx::GlDevice::new(|s| glfw.get_proc_address(s));

    let state = gfx::DrawState::new().depth(gfx::state::LessEqual, true);

    let vertex_data = vec![
        // front (0, 1, 0)
        Vertex { pos: [-1,  1, -1] },
        Vertex { pos: [ 1,  1, -1] },
        Vertex { pos: [ 1,  1,  1] },
    ];

    let mesh = device.create_mesh(vertex_data);
    let slice = mesh.get_slice(gfx::TriangleList);

    let texture_info = gfx::tex::TextureInfo {
        width: 1,
        height: 1,
        depth: 1,
        levels: 1,
        kind: gfx::tex::Texture2D,
        format: gfx::tex::RGBA8,
    };
    let image_info = texture_info.to_image_info();
    let texture = device.create_texture(texture_info).unwrap();
    device.update_texture(&texture, &image_info,
                          &vec![0x20u8, 0xA0u8, 0xC0u8, 0x00u8].as_slice())
        .unwrap();

    let program = device.link_program(VERTEX_SRC.clone(), FRAGMENT_SRC.clone())
                        .unwrap();
    let view: AffineMatrix3<f32> = Transform::look_at(
        &Point3::new(0f32, -5.0, 0.0),
        &Point3::new(0f32, 0.0, 0.0),
        &Vector3::unit_z(),
    );
    let aspect = w as f32 / h as f32;
    let proj = cgmath::perspective(cgmath::deg(45.0f32), aspect, 1.0, 10.0);

    let clear_data = gfx::ClearData {
        color: Some([0.3, 0.3, 0.3, 1.0]),
        depth: Some(1.0),
        stencil: None,
    };

    let mut graphics = gfx::Graphics::new(device);
    let batch: TriangleBatch = graphics.make_batch(&mesh, slice, &program, &state).unwrap();

    while !window.should_close() {
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::KeyEvent(glfw::KeyEscape, _, glfw::Press, _) =>
                    window.set_should_close(true),
                _ => {},
            }
        }

        let start = precise_time_s() * 1000.;
        graphics.clear(clear_data, &frame);

        for x in range(-dimension, dimension) {
            for y in range(-dimension, dimension) {
                let mut model = Matrix3::identity().mul_s(0.01f32).to_matrix4();
                model.w = Vector4::new(x as f32 * 0.05,
                                       0f32,
                                       y as f32 * 0.05,
                                       1f32);

                let data = Params {
                    transform: proj.mul_m(&view.mat)
                                   .mul_m(&model).into_fixed(),
                };
                graphics.draw(&batch, &data, &frame);
            }
        }

        let pre_submit = precise_time_s() * 1000.;
        graphics.end_frame();
        let post_submit = precise_time_s() * 1000.;
        window.swap_buffers();
        let swap = precise_time_s() * 1000.;

        println!("total time:\t\t{0:4.2f}ms", swap - start);
        println!("\tcreate list:\t{0:4.2f}ms", pre_submit - start);
        println!("\tsubmit:\t\t{0:4.2f}ms", post_submit - pre_submit);
        println!("\tgpu wait:\t{0:4.2f}ms", swap - post_submit)
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


fn compile_shader(src: &str, ty: GLenum) -> GLuint {
    let shader = gl::CreateShader(ty);
    unsafe {
        // Attempt to compile the shader
        src.with_c_str(|ptr| gl::ShaderSource(shader, 1, &ptr, ptr::null()));
        gl::CompileShader(shader);

        // Get the compile status
        let mut status = gl::FALSE as GLint;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

        // Fail on error
        if status != (gl::TRUE as GLint) {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::from_elem(len as uint - 1, 0u8);     // subtract 1 to skip the trailing null character
            gl::GetShaderInfoLog(shader, len, ptr::mut_null(), buf.as_mut_ptr() as *mut GLchar);
            fail!("{}", str::from_utf8(buf.as_slice()).expect("ShaderInfoLog not valid utf8"));
        }
    }
    shader
}

fn link_program(vs: GLuint, fs: GLuint) -> GLuint {
    let program = gl::CreateProgram();
    gl::AttachShader(program, vs);
    gl::AttachShader(program, fs);
    gl::LinkProgram(program);
    unsafe {
        // Get the link status
        let mut status = gl::FALSE as GLint;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);

        // Fail on error
        if status != (gl::TRUE as GLint) {
            let mut len: GLint = 0;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::from_elem(len as uint - 1, 0u8);     // subtract 1 to skip the trailing null character
            gl::GetProgramInfoLog(program, len, ptr::mut_null(), buf.as_mut_ptr() as *mut GLchar);
            fail!("{}", str::from_utf8(buf.as_slice()).expect("ProgramInfoLog not valid utf8"));
        }
    }
    program
}

fn gl_main(glfw: glfw::Glfw,
           window: glfw::Window,
           _: Receiver<(f64, glfw::WindowEvent),>,
           dimension: int) {
    // Create GLSL shaders
    let vs = compile_shader(VS_SRC, gl::VERTEX_SHADER);
    let fs = compile_shader(FS_SRC, gl::FRAGMENT_SHADER);
    let program = link_program(vs, fs);

    let mut vao = 0;
    let mut vbo = 0;

    let trans_uniform = unsafe {
        // Create Vertex Array Object
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);

        // Create a Vertex Buffer Object and copy the vertex data to it
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

        let vertex_data = vec![
            // front (0, 1, 0)
            Vertex { pos: [-1,  1, -1] },
            Vertex { pos: [ 1,  1, -1] },
            Vertex { pos: [ 1,  1,  1] },
        ];

        gl::BufferData(gl::ARRAY_BUFFER,
                       (vertex_data.len() * mem::size_of::<Vertex>()) as GLsizeiptr,
                       mem::transmute(&vertex_data[0]),
                       gl::STATIC_DRAW);

        // Use shader program
        gl::UseProgram(program);
        "o_Color".with_c_str(|ptr| gl::BindFragDataLocation(program, 0, ptr));

        // Specify the layout of the vertex data
        let pos_attr = "a_Pos".with_c_str(|ptr| gl::GetAttribLocation(program, ptr));
        gl::EnableVertexAttribArray(pos_attr as GLuint);
        gl::VertexAttribPointer(pos_attr as GLuint, 3, gl::BYTE,
                                gl::FALSE as GLboolean, 0, ptr::null());


        "u_Transform".with_c_str(|ptr|
            gl::GetUniformLocation(program, ptr)
        )
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
        gl::ClearColor(0.3, 0.3, 0.3, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);

        for x in range(-dimension, dimension) {
            for y in range(-dimension, dimension) {
                let mut model = Matrix3::identity().mul_s(0.01f32).to_matrix4();
                model.w = Vector4::new(x as f32 * 0.05,
                                       0f32,
                                       y as f32 * 0.05,
                                       1f32);

                let mat = proj.mul_m(&view.mat).mul_m(&model);

                unsafe {
                    gl::UniformMatrix4fv(trans_uniform,
                                         1,
                                         gl::FALSE,
                                         mat.x.ptr());
                }

                gl::DrawArrays(gl::TRIANGLES, 0, 3);
            }
        }

        let submit = precise_time_s() * 1000.;

        // Swap buffers
        window.swap_buffers();
        let swap = precise_time_s() * 1000.;

        println!("total time:\t\t{0:4.2f}ms", swap - start);
        println!("\tsubmit:\t\t{0:4.2f}ms", submit - start);
        println!("\tgpu wait:\t{0:4.2f}ms", swap - submit)

    }

    // Cleanup
    gl::DeleteProgram(program);
    gl::DeleteShader(fs);
    gl::DeleteShader(vs);
    unsafe {
        gl::DeleteBuffers(1, &vbo);
        gl::DeleteVertexArrays(1, &vao);
    }
}

fn main() {
    let args = os::args();
    if args.len() == 1 {
        println!("gfx-perf [gl|gfx] <size>");
        return;
    }

    let mode = &args[1];
    let count: int = if args.len() >= 2 {
        FromStr::from_str(args[2].as_slice())
    } else {
        None
    }.unwrap_or(10000);

    let count = ((count as f64).sqrt() / 2.) as int;

    let glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    glfw.window_hint(glfw::ContextVersion(3, 2));
    glfw.window_hint(glfw::OpenglForwardCompat(true));
    glfw.window_hint(glfw::OpenglProfile(glfw::OpenGlCoreProfile));

    let (window, events) = glfw
        .create_window(640, 480, "Cube example", glfw::Windowed)
        .expect("Failed to create GLFW window.");

    window.make_current();
    gl::load_with(|s| glfw.get_proc_address(s));
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
