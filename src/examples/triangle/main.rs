extern crate gfx;

#[start]
fn start(argc: int, argv: **u8) -> int {
     native::start(argc, argv, main)
}

fn main() {
    // spawn render task
    let (renderer, platform) = gfx::start(()).unwrap();
    
    // spawn game task
    spawn(proc() {
        let _ = renderer; // do stuff with renderer
        loop {
            
        }
    });

    loop {
        platform.update(); // update platform
    }
}
