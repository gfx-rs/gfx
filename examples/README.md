# Examples

This directory contains a collection of examples which use the various gfx APIs.

The examples are split across three directories, each pertaining to the gfx API they are using.

To run the examples, set your working directory to the examples directory and execute
`cargo run --bin <example> --features=<backend>`, where `<example>` is the example you want to run and `<backend>` is the backend you would like to use (`vulkan`, `dx12`, `metal`, or `gl`).

For example, to run the `quad` example on the `vulkan` backend, try:

    cd examples
    cargo run --bin quad --features=vulkan

If you run the examples for the first time, it may take some time because all dependencies must be compiled too.

## Running `quad` with WebGL/WebAssembly

The quad example also supports WebGL and WebAssembly (`wasm32-unknown-unknown`).

To run the quad example with WebAssembly:

- `cd ..` to move up to the parent directory of the gfx repository
- `git clone https://github.com/grovesNL/spirv_cross` to clone spirv_cross locally
- `cargo install wasm-bindgen-cli` to install wasm-bindgen
- `cd examples` to set the working directory to examples (in the gfx repository)
- `cargo +nightly build --target wasm32-unknown-unknown --features gl --bin quad` to build the quad example to wasm32-unknown-unknown
- `wasm-bindgen ../target/wasm32-unknown-unknown/debug/quad.wasm --out-dir ../examples/generated-wasm --web` to generate wasm bindings
- `cd generated-wasm` to set the working directory to the newly created wasm bindings directory
- `cp ../quad/data/index.html ./` to copy a HTML file containing some simple initialization code to the generated-wasm directory
- `cp ../../../../spirv_cross/wasm/spirv_cross_wrapper_glsl.js ../../../../spirv_cross/wasm/spirv_cross_wrapper_glsl.wasm ./` to copy the spirv_cross JavaScript and WebAssembly files to the generated-wasm directory (alternatively symlink could be used)
- Run any HTTP server supporting `application/wasm` from the `generated-wasm` directory
