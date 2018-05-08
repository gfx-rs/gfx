extern crate gfx_backend_gl as gl;
extern crate gfx_hal as hal;
extern crate gfx_warden as warden;
extern crate ron;
extern crate serde;

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use hal::Instance;
use ron::de::Deserializer;
use serde::de::Deserialize;


fn main() {
    let base_path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../reftests",
    ));

    let raw_scene = {
        let mut raw_data = Vec::new();
        File::open(base_path.join("scenes/basic.ron"))
            .unwrap()
            .read_to_end(&mut raw_data)
            .unwrap();
        let mut deserializer = Deserializer::from_bytes(&raw_data)
            .unwrap();
        warden::raw::Scene::deserialize(&mut deserializer)
            .unwrap()
    };

    let events_loop = gl::glutin::EventsLoop::new();
    let window = gl::glutin::GlWindow::new(
        gl::glutin::WindowBuilder::new(),
        gl::glutin::ContextBuilder::new()
            .with_gl_profile(gl::glutin::GlProfile::Core),
        &events_loop,
        ).unwrap();
    let instance = gl::Surface::from_window(window);

    let adapter = instance.enumerate_adapters().swap_remove(0);
    let mut scene = warden::gpu::Scene::<gl::Backend, _>
        ::new(adapter, &raw_scene, base_path.join("data"))
        .unwrap();
    scene.run(Some("empty"));
    let guard = scene.fetch_image("image.color");
    println!("row: {:?}", guard.row(0));
}
