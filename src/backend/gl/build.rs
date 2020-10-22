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
        glutin: { all(feature = "glutin", not(any(wasm, surfman))) },
        dummy: { not(any(wasm, glutin, surfman)) },
    }

    println!("cargo:rerun-if-changed=build.rs");
}
