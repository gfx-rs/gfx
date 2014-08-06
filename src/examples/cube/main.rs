#![feature(phase)]
#![crate_name = "cube"]

#[phase(plugin)]
extern crate gfx_macros;
extern crate gfx;
extern crate glfw;
extern crate glfw_platform;
extern crate cgmath;
extern crate native;

use cgmath::matrix::{Matrix, Matrix4};
use cgmath::point::Point3;
use cgmath::transform::{Transform, AffineMatrix3};
use cgmath::vector::Vector3;

use glfw_platform::BuilderExtension;

//----------------------------------------
// Cube associated data

#[vertex_format]
struct Vertex {
    #[as_float]
    a_Pos: [i8, ..3],
    #[as_float]
    a_TexCoord: [u8, ..2],
}

impl Vertex {
    fn new(pos: [i8, ..3], tc: [u8, ..2]) -> Vertex {
        Vertex {
            a_Pos: pos,
            a_TexCoord: tc,
        }
    }
}

#[shader_param]
struct Params {
    u_ModelViewProj: [[f32, ..4], ..4],
    t_Color: gfx::shade::TextureParam,
}

static VERTEX_SRC: gfx::ShaderSource = shaders! {
GLSL_120: b"
    #version 120
    attribute vec3 a_Pos;
    attribute vec2 a_TexCoord;
    varying vec2 v_TexCoord;
    uniform mat4 u_ModelViewProj;
    void main() {
        v_TexCoord = a_TexCoord;
        gl_Position = u_ModelViewProj * vec4(a_Pos, 1.0);
    }
"
GLSL_150: b"
    #version 150 core
    in vec3 a_Pos;
    in vec2 a_TexCoord;
    out vec2 v_TexCoord;
    uniform mat4 u_ModelViewProj;
    void main() {
        v_TexCoord = a_TexCoord;
        gl_Position = u_ModelViewProj * vec4(a_Pos, 1.0);
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

    let (mut window, events) = glfw_platform::WindowBuilder::new(&glfw)
        .title("Cube example #gfx-rs")
        .try_modern_context_hints()
        .create()
        .expect("Failed to create GLFW window.");

    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    window.set_key_polling(true); // so we can quit when Esc is pressed
    let (width, height) = window.get_size();

    let (renderer, mut device) = gfx::build()
        .with_glfw(&glfw, window.render_context())
        .with_queue_size(1)
        .create()
        .unwrap();

    spawn(proc() {
        let mut renderer = renderer;
        let frame = gfx::Frame::new(width as u16, height as u16);
        let state = gfx::DrawState::new().depth(gfx::state::LessEqual, true);

        let vertex_data = vec![
            //top (0, 0, 1)
            Vertex::new([-1, -1,  1], [0, 0]),
            Vertex::new([ 1, -1,  1], [1, 0]),
            Vertex::new([ 1,  1,  1], [1, 1]),
            Vertex::new([-1,  1,  1], [0, 1]),
            //bottom (0, 0, -1)
            Vertex::new([ 1,  1, -1], [0, 0]),
            Vertex::new([-1,  1, -1], [1, 0]),
            Vertex::new([-1, -1, -1], [1, 1]),
            Vertex::new([ 1, -1, -1], [0, 1]),
            //right (1, 0, 0)
            Vertex::new([ 1, -1, -1], [0, 0]),
            Vertex::new([ 1,  1, -1], [1, 0]),
            Vertex::new([ 1,  1,  1], [1, 1]),
            Vertex::new([ 1, -1,  1], [0, 1]),
            //left (-1, 0, 0)
            Vertex::new([-1,  1,  1], [0, 0]),
            Vertex::new([-1, -1,  1], [1, 0]),
            Vertex::new([-1, -1, -1], [1, 1]),
            Vertex::new([-1,  1, -1], [0, 1]),
            //front (0, 1, 0)
            Vertex::new([-1,  1, -1], [0, 0]),
            Vertex::new([ 1,  1, -1], [1, 0]),
            Vertex::new([ 1,  1,  1], [1, 1]),
            Vertex::new([-1,  1,  1], [0, 1]),
            //back (0, -1, 0)
            Vertex::new([ 1, -1,  1], [0, 0]),
            Vertex::new([-1, -1,  1], [1, 0]),
            Vertex::new([-1, -1, -1], [1, 1]),
            Vertex::new([ 1, -1, -1], [0, 1]),
        ];

        let mesh = renderer.create_mesh(vertex_data);

        let slice = {
            let index_data = vec![
                0u16, 1, 2, 2, 3, 0,    //top
                4, 5, 6, 6, 7, 4,       //bottom
                8, 9, 10, 10, 11, 8,    //right
                12, 13, 14, 14, 16, 12, //left
                16, 17, 18, 18, 19, 16, //front
                20, 21, 22, 22, 23, 20, //back
            ];

            let buf_index = renderer.create_buffer(Some(index_data));
            gfx::IndexSlice(buf_index, 0, 36)
        };

        let tinfo = gfx::tex::TextureInfo {
            width: 1,
            height: 1,
            depth: 1,
            mipmap_range: (0, 1),
            kind: gfx::tex::Texture2D,
            format: gfx::tex::RGBA8,
        };
        let img_info = tinfo.to_image_info();
        let texture = renderer.create_texture(tinfo);
        renderer.update_texture(texture, img_info, vec![0x20u8, 0xA0u8, 0xC0u8, 0x00u8]);

        let sampler = renderer.create_sampler(gfx::tex::SamplerInfo::new(
            gfx::tex::Bilinear, gfx::tex::Clamp));

        let mut prog = {
            let data = Params {
                u_ModelViewProj: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
                t_Color: (texture, Some(sampler)),
            };
            let handle = renderer.create_program(VERTEX_SRC.clone(), FRAGMENT_SRC.clone());
            renderer.connect_program(handle, data).unwrap()
        };

        let clear = gfx::ClearData {
            color: Some(gfx::Color([0.3, 0.3, 0.3, 1.0])),
            depth: Some(1.0),
            stencil: None,
        };

        let mut m_model = Matrix4::<f32>::identity();
        let m_viewproj = {
            let mv: AffineMatrix3<f32> = Transform::look_at(
                &Point3::new(1.5f32, -5.0, 3.0),
                &Point3::new(0f32, 0.0, 0.0),
                &Vector3::unit_z()
                );
            let aspect = width as f32 / height as f32;
            let mp = cgmath::projection::perspective(
                cgmath::angle::deg(45f32), aspect, 1f32, 10f32);
            mp.mul_m(&mv.mat)
        };

        while !renderer.should_finish() {
            renderer.clear(clear, frame);
            m_model.x.x = 1.0;
            prog.data.u_ModelViewProj = {
                let m = m_viewproj.mul_m(&m_model);
                [ //TODO: add raw convertion methods to cgmath
                    [m.x.x, m.x.y, m.x.z, m.x.w],
                    [m.y.x, m.y.y, m.y.z, m.y.w],
                    [m.z.x, m.z.y, m.z.z, m.z.w],
                    [m.w.x, m.w.y, m.w.z, m.w.w]
                ]
            };
            renderer.draw(&mesh, slice, &frame, &prog, &state)
                .unwrap();
            renderer.end_frame();
            for err in renderer.errors() {
                println!("Renderer error: {}", err);
            }
        }
    });

    while !window.should_close() {
        glfw.poll_events();
        // quit when Esc is pressed.
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::KeyEvent(glfw::KeyEscape, _, glfw::Press, _) => {
                    window.set_should_close(true);
                },
                _ => {},
            }
        }
        device.update();
    }
    device.close();
}
