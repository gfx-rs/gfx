#![feature(phase)]
#![crate_name = "frame"]

extern crate cgmath;
extern crate gfx;
#[phase(plugin)]
extern crate gfx_macros;
extern crate glfw;
extern crate native;
extern crate time;

use cgmath::FixedArray;
use cgmath::{Matrix, Matrix4, Point3, Vector3};
use cgmath::{Transform, AffineMatrix3};
use gfx::{Device, DeviceHelper, TextureHandle, PlaneEmpty, PlaneTexture, Level};
use glfw::Context;

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

#[shader_param(Program)]
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

    glfw.window_hint(glfw::ContextVersion(3, 2));
    glfw.window_hint(glfw::OpenglForwardCompat(true));
    glfw.window_hint(glfw::OpenglProfile(glfw::OpenGlCoreProfile));

    let (window, events) = glfw
        .create_window(640, 480, "Cube example", glfw::Windowed)
        .expect("Failed to create GLFW window.");

    window.make_current();
    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    window.set_key_polling(true); // so we can quit when Esc is pressed
    let (w, h) = window.get_framebuffer_size();
    let frame = gfx::Frame::new(w as u16, h as u16);

    let mut device = gfx::GlDevice::new(|s| glfw.get_proc_address(s));
    let mut list = device.create_draw_list();

    let state = gfx::DrawState::new().depth(gfx::state::LessEqual, true);

    let (gbuffer, t_color) = create_gbuffer(w as u16, h as u16, &mut device);

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

    let mesh = device.create_mesh(vertex_data);

    let slice = {
        let index_data = vec![
            0u8, 1, 2, 2, 3, 0,    //top
            4, 5, 6, 6, 7, 4,       //bottom
            8, 9, 10, 10, 11, 8,    //right
            12, 13, 14, 14, 16, 12, //left
            16, 17, 18, 18, 19, 16, //front
            20, 21, 22, 22, 23, 20, //back
        ];

        let buf = device.create_buffer_static(&index_data);
        gfx::IndexSlice8(buf, 0, 36)
    };

    let tinfo = gfx::tex::TextureInfo {
        width: 2,
        height: 2,
        depth: 1,
        mipmap_range: (0, 1),
        kind: gfx::tex::Texture2D,
        format: gfx::tex::RGBA8,
    };
    let img_info = tinfo.to_image_info();
    let texture = device.create_texture(tinfo).unwrap();
    device.update_texture(&texture, &img_info,
                          &vec![0xFFu8, 0x00u8, 0x00u8, 0x00u8,
                                0x00u8, 0xFFu8, 0x00u8, 0x00u8,
                                0x00u8, 0x00u8, 0xFFu8, 0x00u8,
                                0xFFu8, 0xFFu8, 0xFFu8, 0x00u8])
           .unwrap();

    let sampler = device.create_sampler(gfx::tex::SamplerInfo::new(
        gfx::tex::Scale, gfx::tex::Clamp));

    let prog: Program = device.link_program(VERTEX_SRC.clone(),
        FRAGMENT_SRC.clone()).unwrap();

    let mut m_model = Matrix4::<f32>::identity();
    let m_viewproj = {
        let mv: AffineMatrix3<f32> = Transform::look_at(
            &Point3::new(2f32, -5.0, 3.0),
            &Point3::new(0f32, 0.0, 0.0),
            &Vector3::unit_z()
        );
        let aspect = w as f32 / h as f32;
        let mp = cgmath::perspective(cgmath::deg(45f32),
                                     aspect, 1f32, 10f32);
        mp.mul_m(&mv.mat)
    };

    let mut data = Params {
        u_ModelViewProj: Matrix4::identity().into_fixed(),
        t_Color: (t_color, Some(sampler)),
    };
    let mut data_gbuffer_pass = Params {
        u_ModelViewProj: Matrix4::identity().into_fixed(),
        t_Color: (texture, Some(sampler)),
    };

    while !window.should_close() {
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
        list.reset();
        list.clear(
            gfx::ClearData {
                color: Some(gfx::Color([0.3, 0.3, 0.3, 1.0])),
                depth: Some(1.0),
                stencil: None,
            },
            &frame
        );
        list.clear(
            gfx::ClearData {
                color: Some(gfx::Color([0.3, 1.0, 0.3, 1.0])),
                depth: Some(1.0),
                stencil: None,
            },
            &gbuffer
        );
        m_model.x.x = 1.0;
        data.u_ModelViewProj = m_viewproj.mul_m(&m_model).into_fixed();
        data_gbuffer_pass.u_ModelViewProj = data.u_ModelViewProj;
        list.draw(&mesh, slice, &gbuffer, (&prog, &data_gbuffer_pass), &state).unwrap();
        list.draw(&mesh, slice, &frame, (&prog, &data), &state).unwrap();
        device.submit(list.as_slice());
        window.swap_buffers();
    }
}



fn create_gbuffer(width: u16, height: u16, renderer: &mut gfx::GlDevice) -> (gfx::Frame, TextureHandle) {
  let mut f_gbuffer = gfx::Frame::new(width as u16, height as u16);

  let tinfo_float = gfx::tex::TextureInfo {
      width: width,
      height: height,
      depth: 1,
      mipmap_range: (0, 1),
      kind: gfx::tex::Texture2D,
      format: gfx::tex::Float(gfx::tex::RGBA, gfx::attrib::F16),
  };
  let tinfo_depth = gfx::tex::TextureInfo {
      width: width,
      height: height,
      depth: 1,
      mipmap_range: (0, 1),
      kind: gfx::tex::Texture2D,
      format: gfx::tex::DEPTH24_STENCIL8,
  };
  let t_color = renderer.create_texture(tinfo_float).unwrap();
  let t_normal = renderer.create_texture(tinfo_float).unwrap();
  let t_depth = renderer.create_texture(tinfo_depth).unwrap();

  f_gbuffer.colors = [
    PlaneTexture(t_color.get_name(), 0 as Level, None),
    PlaneTexture(t_normal.get_name(), 0 as Level, None),
    PlaneEmpty,
    PlaneEmpty,
  ];
  f_gbuffer.depth   = PlaneTexture(t_depth.get_name(), 0, None);
  //f_gbuffer.stencil = f_gbuffer.depth;
  (f_gbuffer, t_color)
}
