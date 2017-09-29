#!/bin/bash
set -ex
if [[ $TRAVIS_RUST_VERSION == "nightly" && $TRAVIS_BRANCH == "staging" ]]; then
  # Do not run bors builds against the nightly compiler.
  # We want to find out about nightly bugs, so they're done in master, but we don't block on them.
  exit
fi
export RUST_BACKTRACE=1

EXCLUDES=""
EXCLUDES+=" --exclude gfx_device_dx11"
EXCLUDES+=" --exclude gfx_backend_dx12"

FEATURES=""
if [[ "$TRAVIS_OS_NAME" == "linux" ]]; then
  export PATH=$PATH:$HOME/deps/bin
  export LIBRARY_PATH=$HOME/deps/usr/lib/x86_64-linux-gnu
  export LD_LIBRARY_PATH=$LIBRARY_PATH

  EXCLUDES+=" --exclude gfx_backend_metal"

  FEATURES+="vulkan"

  # Test the `copy` feature on vulkan
  (cd backend/src/vulkan && cargo test --features copy)

elif [[ "$TRAVIS_OS_NAME" == "osx" ]]; then
  EXCLUDES+=" --exclude gfx_backend_vulkan"

  FEATURES+="metal"
  GLUTIN_HEADLESS_FEATURE="--features headless"

  # Test the indirect argument buffer path on OSX
  (cd examples/core/quad && cargo build --features metal,metal_argument_buffer)
fi

cargo build --all --features "$FEATURES" $EXCLUDES

cargo test --all --features $FEATURES $EXCLUDES
cargo test --all --features "$FEATURES mint serialize" $EXCLUDES
