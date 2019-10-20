#[cfg(all(feature = "glutin", not(target_arch = "wasm32")))]
pub mod glutin;

#[cfg(target_arch = "wasm32")]
pub mod web;

#[cfg(all(feature = "wgl", not(target_arch = "wasm32")))]
pub mod wgl;

#[cfg(not(any(target_arch = "wasm32", feature = "glutin", feature = "wgl")))]
pub mod dummy;
