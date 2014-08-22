#![feature(phase)]
#![crate_name = "terrain"]

extern crate cgmath;
extern crate gfx;
#[phase(plugin)]
extern crate gfx_macros;
extern crate glfw;
extern crate native;
extern crate time;
extern crate vertex;
extern crate noise;

use cgmath::FixedArray;
use cgmath::{Matrix, Matrix4, Point3, Vector3};
use cgmath::{Transform, AffineMatrix3};
use gfx::{Device, DeviceHelper};
use glfw::Context;
use vertex::{QuadPipeline, ToTriangles, TrianglePipeline};
use vertex::generators::Plane;
use time::precise_time_s;

use noise::source::Perlin;
use noise::source::Source;

//----------------------------------------
// Cube associated data

#[vertex_format]
struct Vertex {
    // The attributes in here should match up with the attribute/in arguments of
    // the vertex shader.
    a_Pos: [f32, ..3],
    a_Color: [f32, ..3]
}

impl Vertex {
    fn new(pos: [f32, ..3], color: [f32, ..3]) -> Vertex {
        Vertex {
            a_Pos: pos,
            a_Color: color,
        }
    }
}

impl Clone for Vertex {
    fn clone(&self) -> Vertex {
        Vertex {
            a_Pos: self.a_Pos,
            a_Color: self.a_Color
        }
    }
}

// The shader_param attribute makes sure the following struct can be used to
// pass parameters to a shader. Its argument is the name of the type that will
// be generated to represent your the program. Search for link_program below, to
// see how it's used.
#[shader_param(MyProgram)]
struct Params {
    u_ModelViewProj: [[f32, ..4], ..4],
}

static VERTEX_SRC: gfx::ShaderSource = shaders! {
GLSL_120: b"
    #version 120
    attribute vec3 a_Pos;
    attribute vec3 a_Color;
    varying vec3 v_Color;
    uniform mat4 u_ModelViewProj;
    void main() {
        v_Color = a_Color;
        gl_Position = u_ModelViewProj * vec4(a_Pos, 1.0);
    }
"
GLSL_150: b"
    #version 150 core
    in vec3 a_Pos;
    in vec3 a_Color;
    out vec3 v_Color;
    uniform mat4 u_ModelViewProj;
    void main() {
        v_Color = a_Color;
        gl_Position = u_ModelViewProj * vec4(a_Pos, 1.0);
    }
"
};

static FRAGMENT_SRC: gfx::ShaderSource = shaders! {
GLSL_120: b"
    #version 120
    varying vec3 v_Color;
    out vec4 o_Color;
    void main() {
        o_Color = vec4(v_Color, 1.);
    }
"
GLSL_150: b"
    #version 150 core
    in vec3 v_Color;
    out vec4 o_Color;
    void main() {
        o_Color = vec4(v_Color, 1.);
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

fn calculate_color(height: f32) -> [f32, ..3] {
    if height > 8. {
        [0.9, 0.9, 0.9] // white
    } else if height > 0. {
        [0.7, 0.7, 0.7] // greay
    } else if height > -5. {
        [0.2, 0.7, 0.2] // green
    } else {
        [0.2, 0.2, 0.7] // blue
    }
}

fn main() {
    let glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    glfw.window_hint(glfw::ContextVersion(3, 2));
    glfw.window_hint(glfw::OpenglForwardCompat(true));
    glfw.window_hint(glfw::OpenglProfile(glfw::OpenGlCoreProfile));

    let (window, events) = glfw
        .create_window(800, 600, "Terrain example", glfw::Windowed)
        .expect("Failed to create GLFW window.");

    window.make_current();
    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    window.set_key_polling(true); // so we can quit when Esc is pressed
    let (w, h) = window.get_framebuffer_size();
    let frame = gfx::Frame::new(w as u16, h as u16);

    let mut device = gfx::GlDevice::new(|s| glfw.get_proc_address(s));
    let mut renderer = device.create_renderer();

    let state = gfx::DrawState::new().depth(gfx::state::LessEqual, true);

    let noise = Perlin::new();

    let vertex_data: Vec<Vertex> = Plane::subdivide(128, 128)
        .vertex(|vertex::Vector2([x, y])| {
            let h = noise.get(x, y, 0.)*32.;
            Vertex::new([-25. * x, 25.* y, h],
                         calculate_color(h))
        })
        .to_triangles()
        .vertices()
        .collect();


    let mesh = device.create_mesh(vertex_data, gfx::TriangleList);
    let slice = mesh.get_slice();

    let prog: MyProgram = device.link_program(VERTEX_SRC.clone(),
        FRAGMENT_SRC.clone()).unwrap();

    let mut data = Params {
        u_ModelViewProj: Matrix4::identity().into_fixed(),
    };

    while !window.should_close() {
        let time = precise_time_s() as f32;

        let x = time.sin();
        let y = time.cos();

        let m_viewproj = {
            let mv: AffineMatrix3<f32> = Transform::look_at(
                &Point3::new(x*32., y*32., 16.0),
                &Point3::new(0f32, 0.0, 0.0),
                &Vector3::unit_z()
            );
            let aspect = w as f32 / h as f32;
            let mp = cgmath::perspective(cgmath::deg(60f32),
                                         aspect, 0.1f32, 1000f32);
            mp.mul_m(&mv.mat)
        };

        glfw.poll_events();
        // quit when Esc is pressed.
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::KeyEvent(glfw::KeyEscape, _, glfw::Press, _) =>
                    window.set_should_close(true),
                _ => {},
            }
        }
        // render
        renderer.reset();
        renderer.clear(
            gfx::ClearData {
                color: Some(gfx::Color([0.3, 0.3, 0.3, 1.0])),
                depth: Some(1.),
                stencil: None,
            },
            &frame
        );

        let mut m_model = Matrix4::<f32>::identity();
        m_model.x.x = 1.0;
        data.u_ModelViewProj = m_viewproj.mul_m(&m_model).into_fixed();
        renderer.draw(&mesh, slice, &frame, (&prog, &data), &state).unwrap();
        device.submit(renderer.as_buffer());
        window.swap_buffers();
    }
}
