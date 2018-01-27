<p align="center">
  <img src="info/logo.png">
</p>
<p align="center">
  <!--a href="https://docs.rs/gfx-hal">
      <img src="https://docs.rs/gfx-hal/badge.svg" alt="Documentation on docs.rs">
  </a-->
  <a href="https://travis-ci.org/gfx-rs/gfx">
      <img src="https://img.shields.io/travis/gfx-rs/gfx/master.svg?style=flat-square" alt="Travis Build Status">
  </a>
  <a href="https://ci.appveyor.com/project/kvark/gfx">
      <img src="https://ci.appveyor.com/api/projects/status/ryn5ee3aumpmbw5l?svg=true" alt="AppVeyor Build Status">
  </a>
  <!--a href="https://crates.io/crates/gfx-hal">
      <img src="http://img.shields.io/crates/v/gfx-hal.svg?label=gfx-hal" alt = "gfx-hal on crates.io">
  </a-->
  <a href="https://gitter.im/gfx-rs/gfx">
    <img src="https://img.shields.io/badge/GITTER-join%20chat-green.svg?style=flat-square" alt="Gitter Chat">
  </a>
  <br>
  <strong><a href="http://docs.rs/gfx-hal">Documentation</a> | <a href="http://gfx-rs.github.io/">Blog</a> </strong>
</p>

## gfx-rs

gfx-rs is a graphics abstraction library in Rust. It consists of the following layers/components:
- `gfx-hal`: hardware abstraction layer - a Vulkan-ic mostly unsafe API translating to native graphics backends.
- `gfx-backend-*`: graphics backends for various platforms, include the windowing logic.
- `gfx-warden`: data-driven reference test framework.

## pre-LL

If you are looking for information about the released crates (`gfx_core`, `gfx`, `gfx_device_*`, `gfx_window_`, etc), they are being developed and published from the [pre-ll](https://github.com/gfx-rs/gfx/tree/pre-ll) branch. Code in `master` is a complete rewrite that will be shipped in different crates.

### Features

Native API backends:
- [Vulkan](src/backend/vulkan)
- [Direct3D 12](src/backend/dx12)
- [Metal](src/backend/metal)
- [OpenGL 2.1+/ES2+](src/backend/gl)

### Usage

You can run the examples this way:
```bash
git clone https://github.com/gfx-rs/gfx
cd gfx/examples/hal
cargo run --bin quad --features vulkan
cargo run --bin compute --features dx12 1 2 3 4
```
The native API backend is selected by one of the features: `vulkan`, `dx12`, `metal`, or `gl`.

## License
[License]: #license

This repository is currently in the process of being licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option. Some parts of the repository are already licensed according to those terms. See the [tracking issue](https://github.com/gfx-rs/gfx/issues/847).

### Contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
