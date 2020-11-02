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
        dummy: { not(any(wasm, surfman)) },
    }

    println!("cargo:rerun-if-changed=build.rs");
}
