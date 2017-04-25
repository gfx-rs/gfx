<!--
    Copyright 2014 The Gfx-rs Developers.

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
-->
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
      <img src="http://meritbadge.herokuapp.com/gfx?style=flat-square" alt="crates.io">
  </a>
  <a href="https://gitter.im/gfx-rs/gfx">
    <img src="https://img.shields.io/badge/GITTER-join%20chat-green.svg?style=flat-square" alt="Gitter Chat">
  </a>
  <br>
  <strong><a href="http://docs.rs/gfx">Documentation</a> | <a href="https://wiki.alopex.li/LearningGfx">Tutorial</a> | <a href="http://gfx-rs.github.io/">Blog</a> </strong>
</p>

## gfx-rs
`gfx` is a high-performance, bindless graphics API for the Rust programming language. It aims to be the default API for Rust graphics: for one-off applications, or higher level libraries or engines.

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
  - Piston engine - [2d graphics](https://github.com/PistonDevelopers/gfx_graphics)
  - [LazyBox](https://github.com/lazybox/lazybox) engine
  - [Zone of Control](https://github.com/ozkriff/zoc) game
  - [Vange-rs](https://github.com/kvark/vange-rs) game
  - [Claymore](https://github.com/kvark/claymore) game/engine

Shiny screens, including some older projects:
<p align="center">
  <!--img src="https://raw.githubusercontent.com/csherratt/snowmew/master/.screenshot.jpg" height="160" alt="Snowmew"/-->
  <img src="https://github.com/PistonDevelopers/hematite/blob/master/screenshot.png" height="160" alt="Hematite"/>
  <img src="http://image.prntscr.com/image/2f1ec5d477e042dda2c29323c9f49ab4.png" height="160" alt="LazyBox"/>
  <img src="https://github.com/kvark/vange-rs/blob/master/etc/shots/Road10-debug-shape.png" height="160" alt="Vange-rs"/>
  <img src="https://github.com/kvark/claymore/raw/master/etc/screens/7-forest.jpg" height="160" alt="Claymore"/>
  <img src="https://camo.githubusercontent.com/fb8c95650fba27061e58e76f17ff8460a41b3312/687474703a2f2f692e696d6775722e636f6d2f504f68534c77682e706e67" height="160" alt="ZoC"/>
  <!--img src="https://raw.githubusercontent.com/csherratt/petri/master/petri.png" height="160" alt="Petri"/-->
</p>

## Getting started

The gfx-rs git repository contains a number of examples.
Those examples are automatically downloaded if you clone the gfx directory:

	$ cd <my_dir>
	$ git clone https://github.com/gfx-rs/gfx

where `<my_dir>` is a directory name of your choice. Once gfx is downloaded you can build any of the gfx examples.
The examples are listed in the `<my_dir>/gfx/Cargo.toml` file.
For example try:

	$ cd gfx
	$ cargo run --example cube

If you compile the example for the first time, it may take some while since all dependencies must be compiled too.

If you want to build your own stand-alone gfx program, add the following to your new `Cargo.toml`:

	[dependencies]
	gfx = "0.15"


For gfx to work, it needs access to the graphics system of the OS. This is typically provided through some window initialization API.
gfx can use a couple of those to acquire graphical contexts.
For example; [glfw](https://github.com/PistonDevelopers/glfw-rs) or [glutin](https://github.com/tomaka/glutin/).

To see how the graphic context is acquired, see the [cube example](https://github.com/gfx-rs/gfx/tree/master/examples/cube) or the [triangle example](https://github.com/gfx-rs/gfx/tree/master/examples/triangle).

To use `glutin`, for example, your `Cargo.toml` must be extended with the following dependencies:

	[dependencies]
	...
	glutin ="*"
	gfx_window_glutin = "*"

You may want to inspect `<my_dir>/gfx/Cargo.toml` for other crates typically used in gfx programs.

Alternatively, an excellent introduction into gfx and its related crates can be found [here](https://wiki.alopex.li/LearningGfx).

## Note

gfx is still in development. API may change with new backends/features to be implemented.
If you are interested in helping out, checkout [contrib.md](info/contrib.md) and do not hesitate to contact the developers on [Gitter](https://gitter.im/gfx-rs/gfx).
