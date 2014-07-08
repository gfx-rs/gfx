#![feature(phase)]

#[phase(link, plugin)]
extern crate gfx;
extern crate glfw;

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

#[cfg(not(target_os="macos"))]
fn set_hints(glfw: &glfw::Glfw) {
    glfw.window_hint(glfw::ContextVersion(2, 1));
}

// OS X doesn't support VAOs in 2.1 contexts
#[cfg(target_os="macos")]
fn set_hints(glfw: &glfw::Glfw) {
    glfw.window_hint(glfw::ContextVersion(3, 2));
    glfw.window_hint(glfw::OpenglForwardCompat(true));
    glfw.window_hint(glfw::OpenglProfile(glfw::OpenGlCoreProfile));
}

fn main() {
    let glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    set_hints(&glfw);

    let (mut window, events) = glfw
        .create_window(300, 300, "Hello this is window", glfw::Windowed)
        .expect("Failed to create GLFW window.");

    window.set_key_polling(true);
    println!("{}", window.get_context_version());

    // spawn render task
    let (renderer, mut device) = {
        let (platform, provider) = gfx::GlfwPlatform::new(window.render_context(), &glfw);
        gfx::start(platform, provider, 1).unwrap()
    };

    // spawn game task
    spawn(proc() {
        let mut renderer = renderer.unwrap();
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
            let buf = renderer.create_vertex_buffer(data);
            gfx::Constructor::new(buf).
                add("a_Pos", 2, "f32").
                complete(3)
        };
        while !renderer.should_finish() {
            let cdata = gfx::ClearData {
                color: Some(gfx::Color([0.3, 0.3, 0.3, 1.0])),
                depth: None,
                stencil: None,
            };
            renderer.clear(cdata, frame);
            renderer.draw(&mesh, gfx::VertexSlice(0, 3), frame, program, env, state);
            renderer.end_frame();
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
