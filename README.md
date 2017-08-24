<p align="center">
  <img src="info/logo.png">
</p>
<p align="center">
  <a href="https://docs.rs/gfx">
      <img src="https://docs.rs/gfx/badge.svg" alt="Documentation on docs.rs">
  </a>
  <a href="https://travis-ci.org/gfx-rs/gfx">
      <img src="https://img.shields.io/travis/gfx-rs/gfx/master.svg?style=flat-square" alt="Travis Build Status">
  </a>
  <a href="https://ci.appveyor.com/project/kvark/gfx">
      <img src="https://ci.appveyor.com/api/projects/status/ryn5ee3aumpmbw5l?svg=true" alt="AppVeyor Build Status">
  </a>
  <a href="https://crates.io/crates/gfx">
      <img src="http://img.shields.io/crates/v/gfx.svg?label=gfx" alt = "gfx on crates.io">
  </a>
  <a href="https://gitter.im/gfx-rs/gfx">
    <img src="https://img.shields.io/badge/GITTER-join%20chat-green.svg?style=flat-square" alt="Gitter Chat">
  </a>
  <br>
  <strong><a href="http://docs.rs/gfx">Documentation</a> | <a href="https://wiki.alopex.li/LearningGfx">Tutorial</a> | <a href="http://gfx-rs.github.io/">Blog</a> </strong>
</p>

## gfx-rs
`gfx` is a high-performance, bindless graphics API for the Rust programming language. It aims to be the default API for Rust graphics: for one-off applications, or higher level libraries or engines.

### Under Construction

gfx-rs is undergoing severe changes now with transition to the new [low-level core](http://gfx-rs.github.io/2017/07/24/low-level.html). All the following sections apply to the `pre-ll` branch and will be outdated soon. The new development focus is on zero-cost low-level abstraction of current-gen graphics APIs, such as Vulkan, D3D12, and Metal.

---

## Motivation

- Graphics APIs are mostly designed with C and C++ in mind, and hence are dangerous and error prone, with little static safety guarantees.
- Providing type safe wrappers around platform-specific APIs is feasible, but only pushes the problem of platform independence to a higher level of abstraction, often to the game or rendering engine.
- Modern graphics APIs, whilst providing a great degree of flexibility and a high level of performance, often have a much higher barrier to entry than traditional [fixed-function](https://en.wikipedia.org/wiki/Fixed-function) APIs.
- Graphics APIs like OpenGL still [require the developer to 'bind' and 'unbind' objects](https://www.khronos.org/opengl/wiki/Buffer_Object) in order to perform operations on them. This results in a large amount of boiler plate code, and brings with it the usual problems associated with global state.

## Features

Graphics backends:
  - [OpenGL 2.1+](src/backend/gl)
  - [OpenGL ES2+](src/backend/gl) ([works](https://github.com/gfx-rs/gfx/pull/993) on Android)
  - [Direct3D 11](src/backend/dx11)
  - [Metal](src/backend/metal) (WIP 75%)
  - [Vulkan](src/backend/vulkan) (WIP 40%)

Hardware features:
  - [x] off-screen render targets
  - [x] multisampling
  - [x] instancing
  - [x] geometry shaders
  - [x] tessellation
  - [ ] computing
  - [x] persistent mapping

## Who's using it?

Biggest open-source projects are:
  - [Amethyst](https://github.com/amethyst/amethyst) engine
  - [ggez](https://github.com/ggez/ggez) engine
  - Piston engine - [2d graphics](https://github.com/PistonDevelopers/gfx_graphics)
  - [LazyBox](https://github.com/lazybox/lazybox) engine
  - [Vange-rs](https://github.com/kvark/vange-rs) game
  - [Zemeroth](https://github.com/ozkriff/zemeroth) game
  - [Rust-quake](https://github.com/Thinkofname/rust-quake) level viewer
  - [Rust-oids](https://github.com/itadinanta/rust-oids) game

Shiny screens, including some older projects:
<p align="center">
  <!--img src="https://raw.githubusercontent.com/csherratt/snowmew/master/.screenshot.jpg" height="160" alt="Snowmew"/-->
  <img src="https://github.com/PistonDevelopers/hematite/blob/master/screenshot.png" height="160" alt="Hematite"/>
  <img src="http://image.prntscr.com/image/2f1ec5d477e042dda2c29323c9f49ab4.png" height="160" alt="LazyBox"/>
  <img src="https://github.com/kvark/vange-rs/blob/master/etc/shots/Road10-debug-shape.png" height="160" alt="Vange-rs"/>
  <img src="https://github.com/kvark/claymore/raw/master/etc/screens/7-forest.jpg" height="160" alt="Claymore"/>
  <img src="https://camo.githubusercontent.com/fb8c95650fba27061e58e76f17ff8460a41b3312/687474703a2f2f692e696d6775722e636f6d2f504f68534c77682e706e67" height="160" alt="ZoC"/>
  <img src="https://camo.githubusercontent.com/0038d5e3c73b280cfa5d01b26ccef12be7237af5/687474703a2f2f692e696d6775722e636f6d2f703163654954352e706e67" height="160" alt="Rust-Quake">
  <img src="https://github.com/itadinanta/rust-oids/raw/master/img/screenshot_007.png" height="160" alt="Rust-oids">
  <!--img src="https://raw.githubusercontent.com/csherratt/petri/master/petri.png" height="160" alt="Petri"/-->
</p>

## Getting started

If you want to build your own stand-alone gfx program, add the following to your new `Cargo.toml`:

	[dependencies]
	gfx = "0.16"

or, if you want the absolute latest commits to master, you can instead add.

	[dependencies]
	gfx = { git = "https://github.com/gfx-rs/gfx.git" }

For gfx to work, it needs access to the graphics system of the OS. This is typically provided through some window initialization API.
gfx can use a couple of those to acquire graphical contexts.
For example; [glfw](https://github.com/PistonDevelopers/glfw-rs) or [glutin](https://github.com/tomaka/glutin/).

To see how the graphic context is acquired, see the [cube example](https://github.com/gfx-rs/gfx/tree/master/support/examples/cube) or the [triangle example](https://github.com/gfx-rs/gfx/tree/master/render/examples/triangle).

To use `glutin`, for example, your `Cargo.toml` must be extended with the following dependencies:

	[dependencies]
	...
	glutin ="*"
	gfx_window_glutin = "*"

Alternatively, an excellent introduction into gfx and its related crates can be found [here](https://wiki.alopex.li/LearningGfx).

## Running the Examples

The [examples directory](./examples) contains all the examples for GFX, as well as the
[accompanying documentation](./examples/README.md) for understanding and running those examples.


## Structure and current versions
`gfx` consist of several crates. You can find all of them in this repository.

| Core functionality: | Graphic backends: | Window backends: |
| :---: | :---: | :---: |
| [![gfx on crates.io](http://img.shields.io/crates/v/gfx.svg?label=gfx)](http://crates.io/crates/gfx) | [![gfx_device_gl on crates.io](http://img.shields.io/crates/v/gfx_device_gl.svg?label=gfx_device_gl)](http://crates.io/crates/gfx_device_gl) | [![gfx_window_sdl on crates.io](http://img.shields.io/crates/v/gfx_window_sdl.svg?label=gfx_window_sdl)](http://crates.io/crates/gfx_window_sdl) |
| | [![gfx_device_dx11 on crates.io](http://img.shields.io/crates/v/gfx_device_dx11.svg?label=gfx_device_dx11)](http://crates.io/crates/gfx_device_dx11) | [![gfx_window_dxgi on crates.io](http://img.shields.io/crates/v/gfx_window_dxgi.svg?label=gfx_window_dxgi)](http://crates.io/crates/gfx_window_dxgi) |
| [![gfx_core on crates.io](http://img.shields.io/crates/v/gfx_core.svg?label=gfx_core)](http://crates.io/crates/gfx_core) | [![gfx_device_metal on crates.io](http://img.shields.io/crates/v/gfx_device_metal.svg?label=gfx_device_metal)](http://crates.io/crates/gfx_device_metal) | [![gfx_window_glfw on crates.io](http://img.shields.io/crates/v/gfx_window_glfw.svg?label=gfx_window_glfw)](http://crates.io/crates/gfx_window_glfw) |
| [![gfx_macros on crates.io](http://img.shields.io/crates/v/gfx_macros.svg?label=gfx_macros)](http://crates.io/crates/gfx_macros) | [![gfx_device_vulkan on crates.io](http://img.shields.io/crates/v/gfx_device_vulkan.svg?label=gfx_device_vulkan)](http://crates.io/crates/gfx_device_vulkan) | [![gfx_window_metal on crates.io](http://img.shields.io/crates/v/gfx_window_metal.svg?label=gfx_window_metal)](http://crates.io/crates/gfx_window_metal) |
| | | [![gfx_window_glutin on crates.io](http://img.shields.io/crates/v/gfx_window_glutin.svg?label=gfx_window_glutin)](http://crates.io/crates/gfx_window_glutin) |
| | | [![gfx_window_vulkan on crates.io](http://img.shields.io/crates/v/gfx_window_vulkan.svg?label=gfx_window_vulkan)](http://crates.io/crates/gfx_window_vulkan) |

## Note

`gfx` is still in development. API may change with new backends/features to be implemented.
If you are interested in helping out, checkout [contrib.md](info/contrib.md) and do not hesitate to contact the developers on [Gitter](https://gitter.im/gfx-rs/gfx).
