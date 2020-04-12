use gl_generator::{Api, Fallbacks, Profile, Registry};
use std::env;
use std::fs::File;
use std::path::PathBuf;

fn main() {
    // Setup cfg aliases
    cfg_aliases::cfg_aliases! {
        // Platforms
        wasm: { target_arch = "wasm32" },
        android: { target_os = "android" },
        macos: { target_os = "macos" },
        ios: { target_os = "ios" },
        linux: { target_os = "linux" },
        // Backends
        surfman: { all(unix, feature = "surfman", not(ios)) },
        wgl: { all(windows, feature = "wgl") },
        glutin: { all(feature = "glutin", not(any(wasm, surfman))) },
        dummy: { not(any(wasm, glutin, wgl, surfman)) },
    }

    let target = env::var("TARGET").unwrap();
    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=build.rs");

    if target.contains("windows") {
        let mut file = File::create(&dest.join("wgl_sys.rs")).unwrap();
        Registry::new(Api::Wgl, (1, 0), Profile::Core, Fallbacks::All, [])
            .write_bindings(gl_generator::StaticGenerator, &mut file)
            .unwrap();

        let mut file = File::create(&dest.join("wgl_ext_sys.rs")).unwrap();
        Registry::new(
            Api::Wgl,
            (1, 0),
            Profile::Core,
            Fallbacks::All,
            ["WGL_ARB_create_context", "WGL_ARB_pbuffer"],
        )
        .write_bindings(gl_generator::StructGenerator, &mut file)
        .unwrap();
    }
}
