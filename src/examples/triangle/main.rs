extern crate gfx;
extern crate glfw;


static code_vertex: &'static str = "
    #version 150 core
    vec2 pos[3] = vec2[](vec2(-0.5,-0.5), vec2(-0.5,0.5), vec2(0.5,0.5));
    void main() {
        gl_Position = vec4(pos[gl_VertexID], 0.0, 1.0);
    }
";
static code_fragment: &'static str = "
    #version 150 core
    out vec4 o_Color;
    void main() {
        o_Color = vec4(1.0, 0.0, 0.0, 1.0);
    }
";


#[start]
fn start(argc: int, argv: **u8) -> int {
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
            code_vertex.as_bytes().to_owned(),
            code_fragment.as_bytes().to_owned());
        let data = vec!(-0.5f32, -0.5f32, -0.5f32, 0.5f32, 0.5f32, 0.5f32);
        let mesh = renderer.create_mesh(3, data);
        loop {
            renderer.clear(0.3,0.3,0.3);
            renderer.draw(mesh, program);
            renderer.end_frame();
        }
    });

    loop {
        device.update(); // update device
    }
}
