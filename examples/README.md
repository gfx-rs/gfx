# Examples

This directory contains a collection of examples which use the various gfx APIs.

The examples are split across three directories, each pertaining to the gfx API they are using.

To run the examples, set your working directory to the examples directory and execute
`cargo run --bin <example> --features=<backend>`, where `<example>` is the example you want to run and `<backend>` is the backend you would like to use (`vulkan`, `dx12`, `metal`, or `gl`).

For example, to run the `quad` example on the `vulkan` backend, try:

    cd examples
    cargo run --bin quad --features=vulkan

If you run the examples for the first time, it may take some time because all dependencies must be compiled too.
