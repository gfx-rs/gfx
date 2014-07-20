#![feature(phase)]
#![crate_name = "triangle"]

#[phase(plugin)]
extern crate plugin;
extern crate gfx;
extern crate glfw;

use gfx::ToUniform;

#[shader_param]
struct Params {
    b: gfx::BufferHandle,
    x: i32,
    t: gfx::TextureHandle,
}

static VERTEX_SRC: gfx::ShaderSource = shaders! {
GLSL_120: b"
    #version 120
    attribute vec2 a_Pos;
    varying vec4 v_Color;
    void main() {
        v_Color = vec4(a_Pos+0.5, 0.0, 1.0);
        gl_Position = vec4(a_Pos, 0.0, 1.0);
    }
"
GLSL_150: b"
    #version 150 core
    in vec2 a_Pos;
    out vec4 v_Color;
    void main() {
        v_Color = vec4(a_Pos+0.5, 0.0, 1.0);
        gl_Position = vec4(a_Pos, 0.0, 1.0);
    }
"
};

static FRAGMENT_SRC: gfx::ShaderSource = shaders! {
GLSL_120: b"
    #version 120
    varying vec4 v_Color;
    void main() {
        gl_FragColor = v_Color;
    }
"
GLSL_150: b"
    #version 150 core
    in vec4 v_Color;
    out vec4 o_Color;
    void main() {
        o_Color = v_Color;
    }
"
};

#[start]
fn start(argc: int, argv: *const *const u8) -> int {
     native::start(argc, argv, main)
}

fn main() {
    let glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    let (mut window, events) = gfx::glfw::WindowBuilder::new(&glfw)
        .title("Hello this is window")
        .try_modern_context_hints()
        .create()
        .expect("Failed to create GLFW window.");

    glfw.set_error_callback(glfw::FAIL_ON_ERRORS);
    window.set_key_polling(true);

    // spawn render task
    let (renderer, mut device) = {
        let (platform, provider) = gfx::glfw::Platform::new(window.render_context(), &glfw);
        gfx::start(platform, provider, 1).unwrap()
    };

    // spawn game task
    spawn(proc() {
        let mut renderer = renderer;
        let program = renderer.create_program(
            VERTEX_SRC.clone(),
            FRAGMENT_SRC.clone());
        let frame = gfx::Frame::new();
        let mut env = gfx::Environment::new();
        env.add_uniform("color", gfx::ValueF32Vec([0.1, 0.1, 0.1, 0.1]));
        let env = renderer.create_environment(env);
        let state = gfx::DrawState::new();
        let mesh = {
            let data = vec![-0.5f32, -0.5, 0.5, -0.5, 0.0, 0.5];
            let buf = renderer.create_buffer(Some(data));
            gfx::mesh::Builder::new(buf)
                .add("a_Pos", 2, gfx::mesh::F32)
                .complete(3)
        };
        while !renderer.should_finish() {
            let cdata = gfx::ClearData {
                color: Some(gfx::Color([0.3, 0.3, 0.3, 1.0])),
                depth: None,
                stencil: None,
            };
            renderer.clear(cdata, frame);
            renderer.draw(&mesh, gfx::mesh::VertexSlice(0, 3), frame, program, env, state).unwrap();
            renderer.end_frame();
            for err in renderer.iter_errors() {
                println!("Renderer error: {}", err);
            }
        }
    });

    while !window.should_close() {
        glfw.poll_events();
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
