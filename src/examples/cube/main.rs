#![feature(phase)]
#![crate_name = "cube"]

extern crate cgmath;
extern crate gfx;
#[phase(plugin)]
extern crate gfx_macros;
extern crate glfw;
extern crate native;
extern crate time;

use cgmath::FixedArray;
use cgmath::{Matrix, Point3, Vector3};
use cgmath::{Transform, AffineMatrix3};
use gfx::{Device, DeviceHelper, TextureHandle, PlaneTexture, Level};
use glfw::Context;

#[vertex_format]
struct Vertex {
    #[as_float]
    #[name = "a_Pos"]
    pos: [i8, ..3],

    #[as_float]
    #[name = "a_TexCoord"]
    tex_coord: [u8, ..2],
}

// The shader_param attribute makes sure the following struct can be used to
// pass parameters to a shader. Its argument is the name of the type that will
// be generated to represent your the program. Search for `CubeBatch` below, to
// see how it's used.
#[shader_param(CubeBatch)]
struct Params {
    #[name = "u_Transform"]
    transform: [[f32, ..4], ..4],

    #[name = "t_Color"]
    color: gfx::shade::TextureParam,
}

static VERTEX_SRC: gfx::ShaderSource = shaders! {
GLSL_120: b"
    #version 120

    attribute vec3 a_Pos;
    attribute vec2 a_TexCoord;
    varying vec2 v_TexCoord;

    uniform mat4 u_Transform;

    void main() {
        v_TexCoord = a_TexCoord;
        gl_Position = u_Transform * vec4(a_Pos, 1.0);
    }
"
GLSL_150: b"
    #version 150 core

    in vec3 a_Pos;
    in vec2 a_TexCoord;
    out vec2 v_TexCoord;

    uniform mat4 u_Transform;

    void main() {
        v_TexCoord = a_TexCoord;
        gl_Position = u_Transform * vec4(a_Pos, 1.0);
    }
"
};

static FRAGMENT_SRC: gfx::ShaderSource = shaders! {
GLSL_120: b"
    #version 120

    varying vec2 v_TexCoord;
    uniform sampler2D t_Color;

    void main() {
        vec4 tex = texture2D(t_Color, v_TexCoord);
        float blend = dot(v_TexCoord-vec2(0.5,0.5), v_TexCoord-vec2(0.5,0.5));
        gl_FragColor = mix(tex, vec4(0.0,0.0,0.0,0.0), blend*1.0);
    }
"
GLSL_150: b"
    #version 150 core

    in vec2 v_TexCoord;
    out vec4 o_Color;

    uniform sampler2D t_Color;
    void main() {
        vec4 tex = texture(t_Color, v_TexCoord);
        float blend = dot(v_TexCoord-vec2(0.5,0.5), v_TexCoord-vec2(0.5,0.5));
        o_Color = mix(tex, vec4(0.0,0.0,0.0,0.0), blend*1.0);
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

fn main() {
    let glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    glfw.window_hint(glfw::ContextVersion(3, 2));
    glfw.window_hint(glfw::OpenglForwardCompat(true));
    glfw.window_hint(glfw::OpenglProfile(glfw::OpenGlCoreProfile));

    let (window, events) = glfw
        .create_window(640, 480, "Cube example", glfw::Windowed)
        .expect("Failed to create GLFW window.");

    window.make_current();
    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    window.set_key_polling(true);

    let (w, h) = window.get_framebuffer_size();
    let frame = gfx::Frame::new(w as u16, h as u16);

    let mut device = gfx::GlDevice::new(|s| glfw.get_proc_address(s));

    let (gbuffer, texture_color) = create_gbuffer(w as u16, h as u16, &mut device);

    let vertex_data = vec![
        // top (0, 0, 1)
        Vertex { pos: [-1, -1,  1], tex_coord: [0, 0] },
        Vertex { pos: [ 1, -1,  1], tex_coord: [1, 0] },
        Vertex { pos: [ 1,  1,  1], tex_coord: [1, 1] },
        Vertex { pos: [-1,  1,  1], tex_coord: [0, 1] },
        // bottom (0, 0, -1)
        Vertex { pos: [ 1,  1, -1], tex_coord: [0, 0] },
        Vertex { pos: [-1,  1, -1], tex_coord: [1, 0] },
        Vertex { pos: [-1, -1, -1], tex_coord: [1, 1] },
        Vertex { pos: [ 1, -1, -1], tex_coord: [0, 1] },
        // right (1, 0, 0)
        Vertex { pos: [ 1, -1, -1], tex_coord: [0, 0] },
        Vertex { pos: [ 1,  1, -1], tex_coord: [1, 0] },
        Vertex { pos: [ 1,  1,  1], tex_coord: [1, 1] },
        Vertex { pos: [ 1, -1,  1], tex_coord: [0, 1] },
        // left (-1, 0, 0)
        Vertex { pos: [-1,  1,  1], tex_coord: [0, 0] },
        Vertex { pos: [-1, -1,  1], tex_coord: [1, 0] },
        Vertex { pos: [-1, -1, -1], tex_coord: [1, 1] },
        Vertex { pos: [-1,  1, -1], tex_coord: [0, 1] },
        // front (0, 1, 0)
        Vertex { pos: [-1,  1, -1], tex_coord: [0, 0] },
        Vertex { pos: [ 1,  1, -1], tex_coord: [1, 0] },
        Vertex { pos: [ 1,  1,  1], tex_coord: [1, 1] },
        Vertex { pos: [-1,  1,  1], tex_coord: [0, 1] },
        // back (0, -1, 0)
        Vertex { pos: [ 1, -1,  1], tex_coord: [0, 0] },
        Vertex { pos: [-1, -1,  1], tex_coord: [1, 0] },
        Vertex { pos: [-1, -1, -1], tex_coord: [1, 1] },
        Vertex { pos: [ 1, -1, -1], tex_coord: [0, 1] },
    ];

    let mesh = device.create_mesh(vertex_data);

    let index_data: Vec<u8> = vec![
         0,  1,  2,  2,  3,  0, // top
         4,  5,  6,  6,  7,  4, // bottom
         8,  9, 10, 10, 11,  8, // right
        12, 13, 14, 14, 16, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    let slice = {
        let buf = device.create_buffer_static(&index_data);
        gfx::IndexSlice8(gfx::TriangleList, buf, 0, 36)
    };

    let texture_info = gfx::tex::TextureInfo {
        width: 2,
        height: 2,
        depth: 1,
        levels: 1,
        kind: gfx::tex::Texture2D,
        format: gfx::tex::RGBA8,
    };
    let image_info = texture_info.to_image_info();
    let texture = device.create_texture(texture_info).unwrap();
    device.update_texture(&texture, &image_info,
                          &vec![0xFFu8, 0x00u8, 0x00u8, 0x00u8,
                                0x00u8, 0xFFu8, 0x00u8, 0x00u8,
                                0x00u8, 0x00u8, 0xFFu8, 0x00u8,
                                0xFFu8, 0xFFu8, 0xFFu8, 0x00u8])
        .unwrap();

    let sampler = device.create_sampler(
        gfx::tex::SamplerInfo::new(gfx::tex::Scale,
                                   gfx::tex::Clamp)
    );

    let program = device.link_program(VERTEX_SRC.clone(), FRAGMENT_SRC.clone())
                        .unwrap();
    let state = gfx::DrawState::new().depth(gfx::state::LessEqual, true);

    let mut graphics = gfx::Graphics::new(device);
    let batch: CubeBatch = graphics.make_batch(&mesh, slice, &program, &state).unwrap();

    let view: AffineMatrix3<f32> = Transform::look_at(
        &Point3::new(2.5f32, -5.0, 3.0),
        &Point3::new(0f32, 0.0, 0.0),
        &Vector3::unit_z(),
    );
    let aspect = w as f32 / h as f32;
    let proj = cgmath::perspective(cgmath::deg(45.0f32), aspect, 1.0, 10.0);

    let data = Params {
        transform: proj.mul_m(&view.mat).into_fixed(),
        color: (texture_color, Some(sampler)),
    };

    let data_gbuffer_pass = Params {
        transform: proj.mul_m(&view.mat).into_fixed(),
        color: (texture, Some(sampler)),
    };

    let clear_data = gfx::ClearData {
        color: Some([0.3, 0.3, 0.3, 1.0]),
        depth: Some(1.0),
        stencil: None,
    };

    let clear_data_gbuffer = gfx::ClearData {
        color: Some([0.3, 0.7, 0.7, 1.0]),
        depth: Some(1.0),
        stencil: None,
    };

    while !window.should_close() {
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::KeyEvent(glfw::KeyEscape, _, glfw::Press, _) =>
                    window.set_should_close(true),
                _ => {},
            }
        }

        graphics.clear(clear_data_gbuffer, &gbuffer);
        graphics.clear(clear_data, &frame);
        graphics.draw(&batch, &data_gbuffer_pass, &gbuffer);
        graphics.draw(&batch, &data, &frame);
        graphics.end_frame();

        window.swap_buffers();
    }
}

fn create_gbuffer(width: u16, height: u16, renderer: &mut gfx::GlDevice) -> (gfx::Frame, TextureHandle) {
    let mut frame = gfx::Frame::new(width as u16, height as u16);

    let texture_info_float = gfx::tex::TextureInfo {
        width: width,
        height: height,
        depth: 1,
        levels: 1,
        kind: gfx::tex::Texture2D,
        format: gfx::tex::Float(gfx::tex::RGBA, gfx::attrib::F16),
    };
    let texture_info_depth = gfx::tex::TextureInfo {
        width: width,
        height: height,
        depth: 1,
        levels: 1,
        kind: gfx::tex::Texture2D,
        format: gfx::tex::DEPTH24STENCIL8,
    };
    let texture_color = renderer.create_texture(texture_info_float).unwrap();
    let texture_depth = renderer.create_texture(texture_info_depth).unwrap();

    frame.colors[0] = PlaneTexture(texture_color.get_name(), 0 as Level, None);
    frame.depth     = PlaneTexture(texture_depth.get_name(), 0, None);

    (frame, texture_color)
}
