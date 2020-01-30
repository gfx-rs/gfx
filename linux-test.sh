#!/usr/bin/env bash

set -eux

cargo test --all
cargo test -p gfx -p gfx_core --features "serialize mint"
cargo test -p gfx_window_sdl --features "sdl"
cargo test -p gfx_device_gl
cargo test -p gfx_window_glutin --all-features
cargo test -p gfx_window_glfw --features "glfw"
cargo test --all --features vulkan
