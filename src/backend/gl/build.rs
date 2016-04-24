extern crate gl_generator;

use gl_generator::{Registry, Api, Profile, Fallbacks};
use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let dest = Path::new(&out_dir);

    let mut file = File::create(&dest.join("gl_bindings.rs")).unwrap();

    Registry::new(Api::Gl, (4, 5), Profile::Core, Fallbacks::All, [
        "GL_EXT_texture_filter_anisotropic",
        "GL_ARB_draw_buffers_blend",
        ])
        .write_bindings(gl_generator::StructGenerator, &mut file)
        .unwrap();
}
