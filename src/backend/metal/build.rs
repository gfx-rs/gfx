// Compiles the shaders used internally by some commands

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;
use std::ffi::OsStr;

fn main() {
    let pd = env::var("CARGO_MANIFEST_DIR").unwrap();
    let project_dir = Path::new(&pd);
    let shader_dir = project_dir.join("shaders");
    println!("cargo:rerun-if-changed={}", shader_dir.to_str().unwrap());

    let od = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&od);
    let out_lib = out_dir.join("gfx_shaders.metallib");

    // Find all .metal files _at the top level only_
    let shader_files = fs::read_dir(&shader_dir)
        .expect("could not open shader directory")
        .filter_map(|entry| {
            let entry = entry.expect("error reading shader directory entry");
            let path = entry.path();
            match path.extension().and_then(OsStr::to_str) {
                Some("metal") => Some(path),
                _ => None
            }
        });

    // Compile all the metal files into OUT_DIR
    let mut compiled_shader_files: Vec<PathBuf> = Vec::new();
    for shader_path in shader_files.into_iter() {
        println!("cargo:rerun-if-changed={}", shader_path.to_str().unwrap());

        let mut out_path = out_dir.join(shader_path.file_name().unwrap());
        out_path.set_extension("air");

        let status = Command::new("xcrun")
            .args(&["-sdk", "macosx", "metal"])
            .arg(shader_path.as_os_str())
            .arg("-o")
            .arg(out_path.as_os_str())
            .status()
            .expect("failed to execute metal compiler");

        if !status.success() {
            // stdout is linked to parent, so more detailed message will have been output from `metal`
            panic!("shader compilation failed");
        }

        compiled_shader_files.push(out_path);
    }

    // Link all the compiled files into a single library
    let status = Command::new("xcrun")
        .args(&["-sdk", "macosx", "metallib"])
        .args(compiled_shader_files.iter().map(|p| p.as_os_str()))
        .arg("-o")
        .arg(out_lib.as_os_str())
        .status()
        .expect("failed to execute metal library builder");

    if !status.success() {
        panic!("shader library build failed");
    }
}

