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
  <a href="https://travis-ci.org/gfx-rs/gfx">
      <img src="https://img.shields.io/travis/gfx-rs/gfx/master.svg?style=flat-square" alt="Build Status">
  </a>
  <a href="https://crates.io/crates/gfx">
      <img src="http://meritbadge.herokuapp.com/gfx?style=flat-square" alt="crates.io">
  </a>
  <a href="https://gitter.im/gfx-rs/gfx">
    <img src="https://img.shields.io/badge/GITTER-join%20chat-green.svg?style=flat-square" alt="Gitter Chat">
  </a>
  <br>
  <strong><a href="http://gfx-rs.github.io/gfx/gfx/index.html">Documentation</a> | <a href="http://gfx-rs.github.io/">Blog</a> </strong>
</p>

## gfx-rs
`gfx` is a high-performance, bindless graphics API for the Rust programming language. It aims to be the default API for Rust graphics: for one-off applications, or higher level libraries or engines.

## Motivation

- Graphics APIs are mostly designed with C and C++ in mind, and hence are dangerous and error prone, with little static safety guarantees.
- Providing type safe wrappers around platform-specific APIs is feasible, but only pushes the problem of platform independence to a higher level of abstraction, often to the game or rendering engine.
- Modern graphics APIs, whilst providing a great degree of flexibility and a high level of performance, often have a much higher barrier to entry than traditional [fixed-function](https://en.wikipedia.org/wiki/Fixed-function) APIs.
- Graphics APIs like OpenGL still [require the developer to 'bind' and 'unbind' objects](http://www.arcsynthesis.org/gltut/Basics/Intro%20What%20is%20OpenGL.html#d0e887) in order to perform operations on them. This results in a large amount of boiler plate code, and brings with it the usual problems associated with global state.

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

If you compile the the example for the first time, it may take some while since all dependencies must be compiled too.

If you want to build your own stand-alone gfx program, add the following to your new `Cargo.toml`:

	[dependencies]
	gfx = "*"


For gfx to work, it need access to the graphics system of the OS. This is typically provided through the some window initialization API.
gfx can use a couple of those to acquire graphical contexts.
For example; [glfw](https://github.com/PistonDevelopers/glfw-rs) or [glutin](https://github.com/tomaka/glutin/).

To see how the graphic context is acquired, see the [cube example](https://github.com/gfx-rs/gfx/tree/master/examples/cube) or the [triangle example](https://github.com/gfx-rs/gfx/tree/master/examples/triangle).

To use `glfw` or `glutin`, your `Cargo.toml` must be extended with the following dependencies:

	[dependencies]
	...
	glutin ="*"
	gfx_window_glutin = "*"

or

	[dependencies]
	...
	glfw = "*"
	gfx_window_glfw = "*"

You may want to inspect `<my_dir>/gfx/Cargo.toml` for other modules typically used in gfx programs.

## Who's using it?

People are!
![](https://raw.githubusercontent.com/csherratt/snowmew/master/.screenshot.jpg)

## Note

gfx is still in the early stages of development. Help is most appreciated.

If you are interested in helping out, you can contact the developers on [Gitter](https://gitter.im/gfx-rs/gfx). See [contrib.md](info/contrib.md) for contact information and contribution guidelines.
