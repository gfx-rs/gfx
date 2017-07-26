#!/bin/bash
set -ex
export RUST_BACKTRACE=1
if [[ "$TRAVIS_OS_NAME" == "linux" ]]; then
  export PATH=$PATH:$HOME/deps/bin
  export LIBRARY_PATH=$HOME/deps/usr/lib/x86_64-linux-gnu
  export LD_LIBRARY_PATH=$LIBRARY_PATH
  cargo build --features vulkan
elif [[ "$TRAVIS_OS_NAME" == "osx" ]]; then
  GLUTIN_HEADLESS_FEATURE="--features headless"
  cargo build --features metal
else
  cargo build
fi
cargo test --all
cargo test -p gfx -p gfx_core --features "mint serialize"
cargo test -p gfx_window_sdl
cargo test -p gfx_device_gl
cargo test -p gfx_window_glutin $GLUTIN_HEADLESS_FEATURE
cargo test -p gfx_window_glfw
if [[ "$TRAVIS_OS_NAME" == "linux" ]]; then
  cargo test --all --features vulkan
elif [[ "$TRAVIS_OS_NAME" == "linux" ]]; then
  cargo test --all --features metal
  cargo test --all --features "metal metal_argument_buffer"
fi
