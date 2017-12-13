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
- `gfx_hal`: hardware abstraction layer - a Vulkan-ic mostly unsafe API translating to native graphics backends
- `gfx_backend_*`: graphics backends for various platforms, include the windowing logic.
- `gfx_render`: higher level wrapper around HAL, providing resources lifetime tracking, synchronization, and more

The current `master` branch is heavy WIP, please refer to [pre-ll](https://github.com/gfx-rs/gfx/tree/pre-ll) for the latest stable code/examples. It also has a more complete README ;)

### Features

Native API backends:
- [Vulkan](src/backend/vulkan)
- [Direct3D 12](src/backend/dx12)
- [Metal](src/backend/metal)
- (WIP) [OpenGL 2.1+/ES2+](src/backend/gl)

### Usage

TODO
