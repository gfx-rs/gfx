extern crate gfx;
extern crate glfw;

#[start]
fn start(argc: int, argv: **u8) -> int {
     native::start(argc, argv, main)
}

fn main() {
    let glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    let (mut window, events) = glfw
        .create_window(300, 300, "Hello this is window", glfw::Windowed)
        .expect("Failed to create GLFW window.");

    window.set_key_polling(true);
    let platform = gfx::platform::Glfw::new(window.render_context());

    // spawn render task
    let (renderer, device) = gfx::start(platform, ()).unwrap();

    // spawn game task
    spawn(proc() {
        let _ = renderer;
        loop {
            // do stuff with renderer
        }
    });

    loop {
        device.update(); // update device
    }
}
