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
    let (renderer, mut device) = gfx::start(platform, ()).unwrap();

    // spawn game task
    spawn(proc() {
        loop {
            //renderer.clear(0.3,0.3,0.3);
            //renderer.finish();
        }
    });

    loop {
        device.update(); // update device
    }
}
