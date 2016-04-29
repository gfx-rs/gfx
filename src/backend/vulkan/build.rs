extern crate vk_sys;

use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    // tell Cargo that this build script never needs to be rerun
    println!("cargo:rerun-if-changed=build.rs");

    let dest = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&dest);

    let mut file_output = File::create(&dest.join("vk_bindings.rs")).unwrap();
    vk_sys::write_bindings(&mut file_output).unwrap();
}
