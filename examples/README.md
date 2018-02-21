# Examples

This directory contains a collection of examples which use the various gfx APIs.

The examples are split across three directories, each pertaining to the gfx API they are using.

1. `examples/hal` contains low level examples which target the gfx Hardware Abstraction Layer (HAL).
1. `examples/render` shows how to use the render crate directly.
1. `examples/support` shows how to use the support crate, to demonstrate how you can build an application using minimal setup.

_Please note that `support` is still being updated, so `support` examples will not run at the moment._

To run the examples, set your working directory to the examples directory and execute
`cargo run --bin <example> --features=<backend>`, where `<example>` is the example you want to run and `<backend>` is the backend you would like to use (`vulkan`, `dx12`, `metal`, or `gl`).

For example, to run the `quad` example on the `vulkan` backend, try:

    cd examples/hal
    cargo run --bin quad --features=vulkan

If you run the examples for the first time, it may take some time because all dependencies must be compiled too.
