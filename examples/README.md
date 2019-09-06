# Examples

This directory contains a collection of examples which use the various gfx APIs.

The examples are split across three directories, each pertaining to the gfx API they are using.

To run the examples, set your working directory to the examples directory and execute
`cargo run --bin <example> --features=<backend>`, where `<example>` is the example you want to run and `<backend>` is the backend you would like to use (`vulkan`, `dx12`, `metal`, or `gl`).

For example, to run the `quad` example on the `vulkan` backend, try:

```bash
cd examples
cargo run --bin quad --features=vulkan
```

If you run the examples for the first time, it may take some time because all dependencies must be compiled too.

## Running `quad` with WebGL/WebAssembly

The quad example also supports WebGL and WebAssembly (`wasm32-unknown-unknown`).

First, start by compiling the quad example to WebAssembly:

```bash
cd .. # Move up to the parent directory of the gfx repository
git clone https://github.com/grovesNL/spirv_cross # Clone spirv_cross locally
cargo install wasm-bindgen-cli # Install the command line interface (CLI) for wasm-bindgen
cd examples # Set the working directory to examples (in the gfx repository)
cargo +nightly build --target wasm32-unknown-unknown --features gl --bin quad # Build quad as wasm
```

At this point, some crates may fail to build. If they do not build correctly, you may need to update your packages locally by removing your existing Cargo.lock and cleaning your target directory, or forcing certain packages to be updated (i.e. `cargo update -p package-name --precise x.y.z`).

Next, generate bindings and copy across some dependencies: an `index.html` file containing simple initialization code and a JS/wasm bundle for a dependency (spirv_cross):

```bash
wasm-bindgen ../target/wasm32-unknown-unknown/debug/quad.wasm --out-dir ../examples/generated-wasm --web
cd generated-wasm # Set the working directory to the newly created generated-wasm directory
cp ../quad/data/index.html ./ # Copy the index page
cp ../../../../spirv_cross/wasm/spirv_cross*.* ./ # Copy (or symlink) the spirv_cross bundle
```

Afterwards, run any HTTP server supporting `application/wasm` from the `generated-wasm` directory. You may need to add `application/wasm` MIME type to your web server in order for WebAssembly to be served correctly. While the web server is running, open the `index.html` file in your web browser to see the quad render.

## Compiling your own shaders

Now that you've gotten the examples running, you probably want to use your own shaders.

Have a look at [shaderc-rs](https://crates.io/crates/shaderc), [glslang](https://github.com/KhronosGroup/glslang), or [glsl-to-spirv](https://crates.io/crates/glsl-to-spirv)<sup>[deprecated]</sup>.
