extern crate gfx;
extern crate glfw;

static VERTEX_SRC: &'static [u8] = b"
    #version 150 core
    in vec2 a_Pos;
    out vec4 v_Color;
    void main() {
        v_Color = vec4(a_Pos+0.5, 0.0, 1.0);
        gl_Position = vec4(a_Pos, 0.0, 1.0);
    }
";

static FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core
    in vec4 v_Color;
    out vec4 o_Color;
    uniform sampler3D tex3D;
    uniform MyBlock {
        vec4 color;
    } block;
    void main() {
        vec4 texel = texture(tex3D, vec3(0.5,0.5,0.5));
        vec4 unused = mix(texel, block.color, 0.5);
        o_Color = v_Color.x<0.0 ? unused : v_Color;
    }
";

#[start]
fn start(argc: int, argv: *const *const u8) -> int {
     native::start(argc, argv, main)
}

fn main() {
    let glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    glfw.window_hint(glfw::ContextVersion(3, 2));
    glfw.window_hint(glfw::OpenglForwardCompat(true));
    glfw.window_hint(glfw::OpenglProfile(glfw::OpenGlCoreProfile));

    let (mut window, events) = glfw
        .create_window(300, 300, "Hello this is window", glfw::Windowed)
        .expect("Failed to create GLFW window.");

    window.set_key_polling(true);
    let platform = gfx::platform::Glfw::new(window.render_context());

    // spawn render task
    let (renderer, mut device) = gfx::start(platform, &glfw).unwrap();

    // spawn game task
    spawn(proc() {
        let program = renderer.create_program(
            VERTEX_SRC.to_owned(),
            FRAGMENT_SRC.to_owned());
        let data = vec![-0.0f32, 0.5, 0.5, -0.5, -0.5, -0.5];
        let mesh = renderer.create_mesh(3, data, 8);
        loop {
            let cdata = gfx::ClearData {
                color: Some([0.3, 0.3, 0.3, 1.0]),
                depth: None,
                stencil: None,
            };
            renderer.clear(cdata, None);
            renderer.draw(mesh, None, program);
            renderer.end_frame();
        }
    });

    // FIXME: some task fails when the window closes
    while device.update() && !window.should_close() {
        // update device
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::KeyEvent(glfw::KeyEscape, _, glfw::Press, _) => {
                    window.set_should_close(true);
                },
                _ => {},
            }
        }
    }
}
