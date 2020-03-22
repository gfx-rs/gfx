<p align="center">
  <img src="info/logo.png">
</p>
<p align="center">
  <a href="https://matrix.to/#/#gfx:matrix.org">
    <img src="https://img.shields.io/badge/Matrix-%23gfx%3Amatrix.org-blueviolet.svg" alt="Matrix room">
  </a>
  <a href="https://crates.io/crates/gfx-hal">
      <img src="http://img.shields.io/crates/v/gfx-hal.svg?label=gfx-hal" alt = "gfx-hal on crates.io">
  </a>
  <a href="https://travis-ci.org/gfx-rs/gfx">
      <img src="https://img.shields.io/travis/gfx-rs/gfx/master.svg?branch=master" alt="Travis Build Status">
  </a>
  <br>
  <strong><a href="info/getting_started.md">Getting Started</a> | <a href="http://docs.rs/gfx-hal">Documentation</a> | <a href="http://gfx-rs.github.io/">Blog</a> </strong>
</p>

# gfx-rs

gfx-rs is a low-level, cross-platform graphics and compute abstraction library in Rust. It consists of the following components:

* `gfx-hal` which is gfx's hardware abstraction layer: a Vulkan-ic mostly unsafe API which translates to native graphics backends.
* `gfx-backend-*` which contains graphics backends for various platforms:
  * [Vulkan](src/backend/vulkan) (confirmed to run on Linux and Windows)
  * [DirectX 12](src/backend/dx12) and [DirectX 11](src/backend/dx11)
  * [Metal](src/backend/metal) (confirmed to run on macOS and iOS)
  * [OpenGL 2.1+/ES2+](src/backend/gl)
* `gfx-warden` which is a data-driven reference test framework, used to verify consistency across all graphics backends.

gfx-rs is hard to use, it's recommended for performance-sensitive libraries and engines. If that's not your domain, take a look at [wgpu-rs](https://github.com/gfx-rs/wgpu-rs) for a safe and simple alternative.

## The `gfx` crate

This repository was originally home to the [`gfx`](https://crates.io/crates/gfx) crate, which is now deprecated. You can find the latest versions of the code for that crate in the [`pre-ll`](https://github.com/gfx-rs/gfx/tree/pre-ll) branch of this repository.

The master branch of this repository is now focused on developing [`gfx-hal`](https://crates.io/crates/gfx-hal) and its associated backend and helper libraries, as described above. `gfx-hal` is a complete rewrite of `gfx`, but it is not necessarily the direct successor to `gfx`. Instead, it serves a different purpose than the original `gfx` crate, by being "lower level" than the original. Hence, the name of `gfx-hal` was originally `ll`, which stands for "lower level", and the original `gfx` is now referred to as `pre-ll`.

The spiritual successor to the original `gfx` is actually [`wgpu`](https://github.com/gfx-rs/wgpu), which stands on a similar level of abstraction to the old `gfx` crate, but with a modernized API that is more fit for being used over Vulkan/DX12/Metal. If you want something similar to the old `gfx` crate that is being actively developed, `wgpu` is probably what you're looking for, rather than `gfx-hal`.

## Example

To run an example, simply `cd` to the `examples` directory, then use `cargo run` with `--features {backend}` to specify the backend (where `{backend}` is either `vulkan`, `dx12`, `dx11`, `metal`, or `gl`). For example:

```bash
# Clone the gfx repository
git clone https://github.com/gfx-rs/gfx
# Change directory to `examples`
cd gfx/examples
# Vulkan
cargo run --bin quad --features vulkan
# Metal (macOS/iOS)
cargo run --bin quad --features metal
# DirectX12 (Windows)
cargo run --bin compute --features dx12 1 2 3 4
```

This runs the `quad` example using the Vulkan backend, and then the `compute` example using the DirectX 12 backend.

These examples assume that necessary dependencies for the graphics backend are already installed. For more information about installation and usage, refer to the [Getting Started](info/getting_started.md) guide.

## Hardware Abstraction Layer

The Hardware Abstraction Layer (HAL), is a thin, low-level graphics and compute layer which translates API calls to various backends, which allows for cross-platform support. The API of this layer is based on the Vulkan API, adapted to be more Rust-friendly.

<p align="center"><img src="info/hal.svg" alt="Hardware Abstraction Layer (HAL)" /></p>

Currently HAL has backends for Vulkan, DirectX 12/11, Metal, and OpenGL/OpenGL ES/WebGL.

The HAL layer is consumed directly by user applications or libraries. HAL is also used in efforts such as [gfx-portability](https://github.com/gfx-rs/portability).

## Previously Released Crates and API (pre-LL)

The code in `master` is a complete low-level rewrite of gfx based on HAL as described above.

The previously released crates (`gfx_core`, `gfx`, `gfx_device_*`, `gfx_window_`) are still being developed and published from the [pre-ll](https://github.com/gfx-rs/gfx/tree/pre-ll) (pre-low level rewrite) branch.

## Contributing

We are actively looking for new contributors and aim to be welcoming and helpful to anyone that is interested! We know the code base can be a bit intimidating in size and depth at first, and to this end we have a [label](https://github.com/gfx-rs/gfx/issues?q=is%3Aissue+is%3Aopen+label%3Acontributor-friendly) on the issue tracker which marks issues that are new contributor friendly and have some basic direction for completion in the issue comments. If you have any questions about any of these issues (or any other issues) you may want to work on, please comment on GitHub and/or drop a message in our [Matrix chat](https://matrix.to/#/#gfx:matrix.org)!

## License

[license]: #license

This repository is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution Licensing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
